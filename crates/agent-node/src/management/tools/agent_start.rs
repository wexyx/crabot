use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct AgentStart {
    context: Arc<Context>,
}
#[tool(scope="management",name="agent_start",description="Start a business agent, or restart a stopped agent with its existing configuration",parameters=json!({"type":"object","properties":{"client_id":{"type":"string"},"role":{"type":"string"},"provider":{"type":"string","enum":["mock","crabot","codex","claude","opencode"]}},"required":["client_id","role","provider"],"additionalProperties":false}))]
impl AgentStart {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .agent_start(self.context.project(), input.clone())
            .await
    }
}
