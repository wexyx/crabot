use crate::storage::knowledge;
use agent_runtime::context::{HistoryFuture, HistoryQuery, HistorySource};
use serde_json::{Value, json};
use uuid::Uuid;

/// A project's history, reached through the knowledge index.
///
/// The source is bound to one chat and, for a run, one agent. Summaries are stored
/// per `(chat, agent)` because agents differ in context-length limits and compress
/// with their own budgets: a run's window anchors on its own latest summary and
/// still sees records another agent compressed, while the display view (agent
/// `None`) keeps the complete original transcript, independently of compaction.
pub(crate) struct IndexedHistory {
    project: Uuid,
    chat: String,
    agent: Option<String>,
    snapshot: tokio::sync::OnceCell<knowledge::ContextSnapshot>,
    instance: std::path::PathBuf,
}
impl IndexedHistory {
    pub(crate) fn new(project: Uuid, chat: String, agent: Option<String>) -> Self {
        Self {
            project,
            chat,
            agent,
            snapshot: tokio::sync::OnceCell::new(),
            instance: agent_runtime::paths::data_dir(),
        }
    }

    pub(crate) async fn context_rows(
        &self,
        before_message: Option<String>,
    ) -> Result<Vec<Value>, String> {
        let snapshot = self
            .snapshot
            .get_or_try_init(|| async {
                let project = self.project;
                let chat = self.chat.clone();
                let agent = self.agent.clone().ok_or("context requires an Agent")?;
                tokio::task::spawn_blocking(move || {
                    let db = knowledge::for_project(project).ok_or("历史索引不可用")?;
                    knowledge::context_snapshot(&db, &chat, &agent, before_message.as_deref())
                })
                .await
                .map_err(|e| e.to_string())?
            })
            .await?;
        Ok(snapshot.rows.iter().map(hit_to_row).collect())
    }

    pub(crate) async fn context_prompt(
        &self,
        before_message: Option<String>,
    ) -> Result<String, String> {
        let rows = self.context_rows(before_message).await?;
        let records = crate::core::recent_context::select(&rows, usize::MAX)
            .into_iter()
            .map(|row| {
                let mut record = row.get("payload").cloned().unwrap_or_else(|| row.clone());
                record["type"] = row["type"].clone();
                record["seq"] = row["seq"].clone();
                record["is_self"] =
                    json!(self.agent.as_deref().is_some_and(|a| record["agent"] == a));
                record
            })
            .collect::<Vec<_>>();
        if records.is_empty() {
            return Ok(String::new());
        }
        Ok(format!(
            "{}\nPrevious topic records:\n{}\n",
            crate::core::recent_context::NOTICE,
            serde_json::to_string(&records).map_err(|e| e.to_string())?
        ))
    }

    /// Host-only sequential read with full stored payloads.
    ///
    /// Web/CLI display and context-window building need the complete event rows
    /// (tool inputs, message ids), so this bypasses the model-shaped `read_range`
    /// and rebuilds rows from each hit's stored `raw`. `chat` defaults to the
    /// source's own chat. The flag reports whether the queried range held more
    /// records than `limit` (used for pagination).
    pub(crate) async fn rows(
        &self,
        chat: Option<&str>,
        after_seq: u64,
        before_seq: u64,
        limit: usize,
    ) -> Result<(Vec<Value>, bool), String> {
        let project = self.project;
        let chat = chat.map(str::to_owned).unwrap_or_else(|| self.chat.clone());
        let (hits, truncated) = tokio::task::spawn_blocking(move || {
            let db = knowledge::for_project(project).ok_or("历史索引不可用")?;
            knowledge::recall_range(&db, &chat, None, after_seq, before_seq, limit)
        })
        .await
        .map_err(|e| e.to_string())??;
        Ok((hits.iter().map(hit_to_row).collect(), truncated))
    }
}

impl HistorySource for IndexedHistory {
    /// Search across existing conversation indexes in this instance only.
    fn search<'a>(&'a self, query: HistoryQuery) -> HistoryFuture<'a> {
        let instance = self.instance.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                knowledge::HistorySearch::new(&instance).search(query)
            })
            .await
            .map_err(|e| e.to_string())?
        })
    }
    fn read_project_range<'a>(
        &'a self,
        project: Option<&'a str>,
        chat: Option<&'a str>,
        after: u64,
        before: u64,
        limit: usize,
    ) -> HistoryFuture<'a> {
        let Some(project) = project else {
            return self.read_range(chat, after, before, limit);
        };
        let project = project.to_owned();
        let chat = chat.unwrap_or_default().to_owned();
        let instance = self.instance.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                knowledge::HistorySearch::new(&instance).read(&project, &chat, after, before, limit)
            })
            .await
            .map_err(|e| e.to_string())?
        })
    }

    /// Read history by sequence range from the index, in sequential order.
    ///
    /// The model can inspect complete original events, including summarized records.
    fn read_range<'a>(
        &'a self,
        chat: Option<&'a str>,
        after_seq: u64,
        before_seq: u64,
        limit: usize,
    ) -> HistoryFuture<'a> {
        Box::pin(async move {
            let (rows, truncated) = self.rows(chat, after_seq, before_seq, limit).await?;
            Ok(json!({"records":rows,"truncated":truncated}))
        })
    }

    /// Persist a compaction summary for this run's chat and agent.
    ///
    /// Coverage is frozen when the input snapshot is built, not when generation ends.
    fn write_summary<'a>(&'a self, summary: &str) -> HistoryFuture<'a> {
        let project = self.project;
        let chat = self.chat.clone();
        let agent = self
            .agent
            .clone()
            .ok_or_else(|| "当前源不持久化摘要".to_string());
        let summary = summary.to_string();
        Box::pin(async move {
            let agent = agent?;
            let snapshot = self
                .snapshot
                .get()
                .ok_or("context snapshot was not loaded; refusing unsafe summary coverage")?;
            if snapshot.through <= snapshot.reset {
                return Ok(json!({"status":"empty"}));
            }
            let seq_start = snapshot.reset.saturating_add(1);
            let seq_end = snapshot.through;
            tokio::task::spawn_blocking(move || {
                let db = knowledge::for_project(project).ok_or("历史索引不可用")?;
                knowledge::write_summary(&db, &chat, &agent, seq_start, seq_end, &summary)?;
                Ok(json!({"status":"stored","chat":chat,"agent":agent,"through_seq":seq_end}))
            })
            .await
            .map_err(|e| e.to_string())?
        })
    }
}

/// Rebuild a log-shaped row from one index hit.
///
/// The stored `raw` payload wins when present — it is the complete event row. The
/// reduced fields are the fallback for records written before the index kept raw.
fn hit_to_row(hit: &knowledge::Hit) -> Value {
    if let Some(raw) = hit.raw.as_deref() {
        if let Ok(row) = serde_json::from_str::<Value>(raw) {
            return row;
        }
    }
    let mut row = json!({
        "seq": hit.seq,
        "type": hit.kind,
        "payload": {"content": hit.excerpt},
        "logged_at": "",
    });
    if !hit.agent.is_empty() {
        row["payload"]["agent"] = json!(&hit.agent);
    }
    if let Some(message_id) = hit.message_id.as_deref().filter(|s| !s.is_empty()) {
        row["payload"]["message_id"] = json!(message_id);
    }
    row
}
