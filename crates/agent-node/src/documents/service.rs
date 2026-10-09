use agent_runtime::context::{DocumentSource, HistoryFuture};
use serde_json::{Value, json};

pub(crate) struct Documents {
    root: std::path::PathBuf,
}
impl Documents {
    pub(crate) fn new() -> Self {
        Self {
            root: agent_runtime::paths::data_dir(),
        }
    }
    pub(crate) async fn execute(&self, input: Value) -> Result<Value, String> {
        if input["action"] == "import" {
            return super::crawl::import(self, input).await;
        }
        self.store(input).await
    }
    pub(super) async fn store(&self, input: Value) -> Result<Value, String> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            crate::storage::knowledge::DocumentStore::new(root).execute(&input)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    pub(crate) async fn upload(&self, name: String, bytes: Vec<u8>) -> Result<Value, String> {
        let (title, content) = Self::extract(bytes, name.clone(), String::new()).await?;
        self.execute(json!({"action":"save","title":if title.is_empty(){name.clone()}else{title},"content":content,"source":format!("upload:{name}")})).await
    }
    pub(super) async fn extract(
        bytes: Vec<u8>,
        name: String,
        mime: String,
    ) -> Result<(String, String), String> {
        static JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
        let permit = JOBS.acquire().await.map_err(|e| e.to_string())?;
        let result =
            tokio::task::spawn_blocking(move || super::extract::extract(&bytes, &name, &mime))
                .await
                .map_err(|e| e.to_string())?;
        drop(permit);
        result
    }
}
impl DocumentSource for Documents {
    fn upload(&self, name: String, bytes: Vec<u8>) -> HistoryFuture<'_> {
        Box::pin(self.upload(name, bytes))
    }
    fn execute(&self, input: Value) -> HistoryFuture<'_> {
        Box::pin(self.execute(input))
    }
}
