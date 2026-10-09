use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeSet, future::Future, pin::Pin};
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;
#[derive(Clone, Serialize)]
pub struct ToolDefinition {
    name: String,
    description: String,
    parameters: Value,
}
impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn parameters(&self) -> &Value {
        &self.parameters
    }
}
#[derive(Default)]
pub struct ToolSession {
    pending_summary: Option<String>,
    pending_compact: bool,
    /// Tools this model has discovered through `find_tools` and may now be offered.
    unlocked_tools: BTreeSet<String>,
}
impl ToolSession {
    /// Schedule the host-driven summary path: the tool call is only a trigger, the
    /// host compresses the window through a fresh model request and persists the
    /// summary itself.
    pub(crate) fn request_compact(&mut self) {
        self.pending_compact = true;
    }
    pub(crate) fn take_compact(&mut self) -> bool {
        std::mem::take(&mut self.pending_compact)
    }
    /// Schedule the "drop older rounds without a summary" path, using this fixed text.
    pub(crate) fn summarize(&mut self, summary: String) {
        self.pending_summary = Some(summary);
    }
    pub(crate) fn take_summary(&mut self) -> Option<String> {
        self.pending_summary.take()
    }
    /// Reveal a tool for the rest of this run. Discovery is additive and never
    /// widens authorization: a disabled tool is absent from the registry entirely.
    pub(crate) fn unlock(&mut self, name: &str) {
        self.unlocked_tools.insert(name.into());
    }
    pub(crate) fn is_unlocked(&self, name: &str) -> bool {
        self.unlocked_tools.contains(name)
    }
}
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn execute<'a>(&'a self, args: &'a Value, session: &'a mut ToolSession) -> ToolFuture<'a>;
}
