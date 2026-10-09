use super::HistoryFuture;
use serde_json::Value;
use std::{future::Future, sync::Arc};

/// Document capability supplied by the host, bound to its shared instance library.
pub trait DocumentSource: Send + Sync {
    fn execute(&self, input: Value) -> HistoryFuture<'_>;
    fn upload(&self, _name: String, _bytes: Vec<u8>) -> HistoryFuture<'_> {
        Box::pin(async { Err("Document upload unavailable".into()) })
    }
}
tokio::task_local! { static SOURCE: Arc<dyn DocumentSource>; }
pub struct DocumentAccess;
impl DocumentAccess {
    pub async fn scope<F: Future>(source: Arc<dyn DocumentSource>, future: F) -> F::Output {
        SOURCE.scope(source, future).await
    }
    pub async fn execute(input: Value) -> Result<Value, String> {
        let source = SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "Document index not bound to this run")?;
        source.execute(input).await
    }
    pub async fn upload(name: String, bytes: Vec<u8>) -> Result<Value, String> {
        let source = SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "Document index not bound to this run")?;
        source.upload(name, bytes).await
    }
}
