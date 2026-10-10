use super::{graph, recall};
use agent_runtime::context::HistoryQuery;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Read-only discovery of existing indexes in this instance; never follow links or
/// create a database from model-provided paths.
pub(crate) struct HistorySearch {
    root: PathBuf,
}
impl HistorySearch {
    pub(crate) fn new(instance: &Path) -> Self {
        Self {
            root: instance.join("knowledge"),
        }
    }
    fn projects(&self) -> Result<Vec<(Uuid, PathBuf)>, String> {
        if std::fs::symlink_metadata(&self.root).is_ok_and(|m| !m.file_type().is_dir()) {
            return Err("History index root must be a regular directory".into());
        }
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut result = vec![];
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            let Ok(id) = Uuid::parse_str(&entry.file_name().to_string_lossy()) else {
                continue;
            };
            if std::fs::symlink_metadata(entry.path().join("graph.db"))
                .is_ok_and(|m| m.file_type().is_file())
            {
                result.push((id, entry.path()));
            }
        }
        result.sort_by_key(|(id, _)| *id);
        Ok(result)
    }
    fn project(&self, id: &str) -> Result<PathBuf, String> {
        let id =
            Uuid::parse_str(id).map_err(|_| "project must be a UUID returned by history search")?;
        self.projects()?
            .into_iter()
            .find(|(p, _)| *p == id)
            .map(|(_, path)| path)
            .ok_or_else(|| "History project not found in this instance".into())
    }
    pub(crate) fn search(&self, query: HistoryQuery) -> Result<Value, String> {
        if query.query.trim().is_empty() {
            return Err("History search requires a query".into());
        }
        let projects = if let Some(id) = &query.project {
            vec![(
                Uuid::parse_str(id).map_err(|_| "invalid history project")?,
                self.project(id)?,
            )]
        } else {
            self.projects()?
        };
        let limit = query.limit.clamp(1, 100);
        let mut matches = vec![];
        let mut truncated = false;
        let mut unavailable = vec![];
        for (project, path) in projects {
            let result = graph::open(&path)
                .ok_or_else(|| "History index unavailable".to_owned())
                .and_then(|db| recall::recall(&db, &query.query, query.chat.as_deref(), limit));
            match result {
                Ok((hits, more)) => {
                    truncated |= more;
                    for hit in hits {
                        matches.push((project, hit));
                    }
                    matches.sort_by(|a, b| {
                        b.1.score
                            .unwrap_or(0.0)
                            .total_cmp(&a.1.score.unwrap_or(0.0))
                            .then_with(|| a.0.cmp(&b.0))
                            .then_with(|| b.1.seq.cmp(&a.1.seq))
                    });
                    truncated |= matches.len() > limit;
                    matches.truncate(limit);
                }
                Err(error) => unavailable.push(json!({"project":project,"error":error})),
            }
        }
        let rows = matches
            .into_iter()
            .map(|(project, hit)| {
                let mut row = recall::render(&[hit], false)["matches"][0].take();
                row["project"] = json!(project);
                row
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"scope":"instance","matched":rows.len(),"matches":rows,"truncated":truncated,"unavailable":unavailable}),
        )
    }
    pub(crate) fn read(
        &self,
        project: &str,
        chat: &str,
        after: u64,
        before: u64,
        limit: usize,
    ) -> Result<Value, String> {
        if chat.is_empty() {
            return Err("chat is required for an exact project history read".into());
        }
        let db = graph::open(&self.project(project)?).ok_or("History index unavailable")?;
        let (hits, truncated) = recall::recall_range(&db, chat, None, after, before, limit)?;
        let records = hits
            .iter()
            .map(|hit| {
                hit.raw
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .unwrap_or_else(
                        || json!({"seq":hit.seq,"type":hit.kind,"payload":{"content":hit.excerpt}}),
                    )
            })
            .collect::<Vec<_>>();
        Ok(json!({"project":project,"chat":chat,"records":records,"truncated":truncated}))
    }
}

#[cfg(test)]
#[path = "history_search_tests.rs"]
mod tests;
