use super::{ToolContext, ToolSession};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    command: String,
}
struct Shell {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(name="shell",description="Run a shell command on the host in this Agent working directory. Authorization follows the host-selected permission mode; never approve your own actions. Runs with the current OS user permissions, including files outside the working directory and network access. Save generated screenshots, downloads and temporary artifacts under $CRABOT_TMP_DIR, not the working directory root, unless the user explicitly requests another output path. Never bypass an approval denial.",parameters=json!({"type":"object","properties":{"command":{"type":"string","maxLength":8192}},"required":["command"],"additionalProperties":false}),runtime=crate)]
impl Shell {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        context.workdir().is_some().then_some(Self { context })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        super::shell_execution::run(&self.context, &input.command).await
    }
}
