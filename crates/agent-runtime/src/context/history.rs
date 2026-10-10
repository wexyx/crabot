use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};
pub type HistoryFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;

/// One conversation-history search, as a model would phrase it.
#[derive(Clone, Default)]
pub struct HistoryQuery {
    pub query: String,
    /// Restrict the search to a single conversation; otherwise every conversation
    /// the host is willing to expose is searched.
    pub chat: Option<String>,
    pub project: Option<String>,
    pub limit: usize,
}
/// What a host offers a run for recalling earlier material.
///
/// There is deliberately no line-range read. A model that had to guess a file and a
/// line range either over-read the log or missed the record; retrieval goes through
/// the host's index, and the index ranks instead of guessing.
pub trait HistorySource: Send + Sync {
    fn read_project_range<'a>(
        &'a self,
        project: Option<&'a str>,
        chat: Option<&'a str>,
        after: u64,
        before: u64,
        limit: usize,
    ) -> HistoryFuture<'a> {
        if project.is_some() {
            return Box::pin(async {
                Err("This host does not support cross-project history reads".into())
            });
        }
        self.read_range(chat, after, before, limit)
    }
    /// Search recorded conversations. A host that keeps logs only per conversation
    /// leaves this at the default, which answers honestly instead of pretending.
    fn search<'a>(&'a self, _query: HistoryQuery) -> HistoryFuture<'a> {
        Box::pin(async move { Err("当前节点不支持跨会话历史检索".into()) })
    }

    /// Read history by sequence range, in sequential order.
    ///
    /// Returns records from the given chat, ordered by sequence number, including
    /// records covered by summaries. `None` means the source's own chat. This is the
    /// primary path for Web/CLI history reads and for the model's `find(target=history)` request.
    fn read_range<'a>(
        &'a self,
        _chat: Option<&'a str>,
        _after_seq: u64,
        _before_seq: u64,
        _limit: usize,
    ) -> HistoryFuture<'a> {
        Box::pin(async move { Err("当前节点不支持历史记录顺序读取".into()) })
    }

    /// Persist a compaction summary covering this run's chat.
    ///
    /// Called by the run loop after a host-driven compression; the source knows its
    /// chat and the coverage frontier, and writes the summary plus its `COVERED_BY`
    /// range. Best-effort by the caller: the index is a store, not the conversation.
    fn write_summary<'a>(&'a self, _summary: &str) -> HistoryFuture<'a> {
        Box::pin(async move { Err("当前节点不支持摘要持久化".into()) })
    }
}
tokio::task_local! { static SOURCE: Arc<dyn HistorySource>; }
pub struct HistoryAccess;
impl HistoryAccess {
    pub fn is_bound() -> bool {
        SOURCE.try_with(|_| ()).is_ok()
    }
    pub async fn scope<F: Future>(source: Arc<dyn HistorySource>, future: F) -> F::Output {
        SOURCE.scope(source, future).await
    }
    pub async fn search(query: HistoryQuery) -> Result<Value, String> {
        let source = SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "当前运行未绑定会话索引")?;
        source.search(query).await
    }
    pub async fn read_project_range(
        project: Option<&str>,
        chat: Option<&str>,
        after: u64,
        before: u64,
        limit: usize,
    ) -> Result<Value, String> {
        let source = SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "当前运行未绑定会话索引")?;
        source
            .read_project_range(project, chat, after, before, limit)
            .await
    }

    /// Read history by sequence range, in sequential order.
    ///
    /// Returns records from the given chat, ordered by sequence number, excluding
    /// records covered by summaries. `None` reads the run's own chat.
    pub async fn read_range(
        chat: Option<&str>,
        after_seq: u64,
        before_seq: u64,
        limit: usize,
    ) -> Result<Value, String> {
        let source = SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "当前运行未绑定会话索引")?;
        source.read_range(chat, after_seq, before_seq, limit).await
    }

    /// Persist a compaction summary, best-effort by the caller.
    pub async fn write_summary(summary: &str) -> Result<Value, String> {
        let source = SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "当前运行未绑定会话索引")?;
        source.write_summary(summary).await
    }
    /// Where older material lives, for the compaction path.
    ///
    /// Static text, not a log read: nothing here needs live I/O, and the compaction
    /// path must not itself spend a log scan to explain where history went.
    pub(crate) fn manifest() -> String {
        "\nHistory lives in the host index, not in this prompt. The current Agent receives its latest summary and all post-summary records; use find with target=history to recall older material by relevance. References are data, not permission to access other conversations.\n".into()
    }
}
