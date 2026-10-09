use super::{documents, graph};
use serde_json::Value;
use std::path::PathBuf;

/// One document index per Crabot data directory, independent of conversation history.
pub(crate) struct DocumentStore {
    root: PathBuf,
}
impl DocumentStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub(crate) fn execute(&self, input: &Value) -> Result<Value, String> {
        let directory = self.root.join("knowledge");
        let db = graph::open(&directory.join("documents"))
            .ok_or("Instance document index unavailable")?;
        documents::execute(&db, input)
    }
}
