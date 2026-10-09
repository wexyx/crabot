use super::super::{
    config::HarnessConfig,
    turn::{Delta, Turn},
};
use crate::tools::{ToolDefinition, ToolRegistry};
use serde_json::Value;
use std::collections::BTreeMap;
pub(in super::super) trait ModelProtocol: Send + Sync {
    fn image(&self, image: &crate::attachments::AttachmentImage) -> Value;
    fn with_images(&self, history: &[Value]) -> Vec<Value> {
        let mut history = history.to_vec();
        crate::attachments::PreparedAttachments::images(|images| {
            if !images.is_empty() {
                history.push(serde_json::json!({"role":"user","content":images.iter().map(|image| self.image(image)).collect::<Vec<_>>()}));
            }
        });
        history
    }
    fn tool(&self, definition: &ToolDefinition) -> Value;
    fn request(
        &self,
        http: &reqwest::Client,
        config: &HarnessConfig,
        history: &[Value],
        tools: &ToolRegistry,
    ) -> reqwest::RequestBuilder;
    /// `delta` receives every fragment tagged by channel. Answer text becomes the reply;
    /// deliberation is folded into progress and must never join the answer.
    fn consume(
        &self,
        turn: &mut Turn,
        value: Value,
        delta: &mut dyn FnMut(Delta),
    ) -> Result<(), String>;
    fn append_history(
        &self,
        history: &mut Vec<Value>,
        turn: &Turn,
        results: &BTreeMap<usize, String>,
    );
    fn tools(&self, registry: &ToolRegistry) -> Vec<Value> {
        registry
            .definitions()
            .iter()
            .map(|d| self.tool(d))
            .collect()
    }
}
