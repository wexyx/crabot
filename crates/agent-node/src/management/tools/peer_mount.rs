use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct PeerMount {
    context: Arc<Context>,
}
#[tool(scope="management",name="peer_mount",description="Request connecting this node to an upstream Crabot URL. Discovers and invokes Agents; requires explicit human confirmation. Credentials are generated automatically.",parameters=json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"],"additionalProperties":false}))]
impl PeerMount {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .propose_mount(self.context.project(), input.clone())
            .await
    }
}
