use super::{ToolContext, ToolSession};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(default)]
    action: Option<String>,
    command: Option<String>,
    session_id: Option<String>,
    input: Option<String>,
    #[serde(default)]
    interactive: bool,
    #[serde(default)]
    after: u64,
    wait_ms: Option<u64>,
}
struct Shell {
    context: Arc<ToolContext>,
    fallback: crate::execution::SessionScope,
}
#[crate::tools::tool(name="shell",description="Run host shell commands or continue a process session. action=start (default) takes command; interactive=true provides a PTY for login/interactive programs. Returns status and session_id after wait_ms (default 1000, maximum 10000), without killing a running process. read uses session_id and after=next_cursor to get later output; write sends input verbatim (include newline to submit); stop terminates the process group. Sessions belong to this Agent and chat; HOME persists per Agent. For passwords/OTP ask the human to use the process panel, never request secrets in chat or tool input. Read sparingly; yield to the user while awaiting login. All writes follow approval, with no whitelist bypass. Host user file/network permissions apply. Store artifacts under $CRABOT_TMP_DIR.",parameters=json!({"type":"object","properties":{"action":{"type":"string","enum":["start","read","write","stop"]},"command":{"type":"string","maxLength":8192},"interactive":{"type":"boolean"},"session_id":{"type":"string"},"input":{"type":"string","maxLength":8192},"after":{"type":"integer","minimum":0},"wait_ms":{"type":"integer","minimum":0,"maximum":10000}},"additionalProperties":false}),runtime=crate)]
impl Shell {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        let fallback = crate::execution::SessionScope::new(
            "standalone".into(),
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
        )
        .with_data(context.workdir()?.join(".crabot"));
        Some(Self { context, fallback })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        let scope =
            crate::execution::SessionScope::current().unwrap_or_else(|| self.fallback.clone());
        let manager = crate::execution::ProcessSessions::global();
        let action = input.action.as_deref().unwrap_or("start");
        let wait = input
            .wait_ms
            .unwrap_or(if action == "start" { 1000 } else { 0 });
        if wait > 10000 {
            return Err("wait_ms maximum 10000".into());
        }
        if action == "start" {
            if input.session_id.is_some() || input.input.is_some() {
                return Err("start accepts command, not session_id/input".into());
            }
            let command = input.command.ok_or("command required")?;
            super::shell_execution::validate(&command)?;
            let root = self
                .context
                .workdir()
                .ok_or("workspace required")?
                .to_path_buf();
            let profile = self.context.execution.select_profile(None)?;
            crate::workspace::confirm_command(&root, &command, &profile).await?;
            if root.canonicalize().map_err(|e| e.to_string())? != root {
                return Err("workspace changed during approval".into());
            }
            let bridge =
                crate::skills::SkillBridge::start(root.clone(), self.context.skills()).await?;
            let mut env = bridge.as_ref().map(|b| b.environment()).unwrap_or_default();
            env.extend(crate::execution::profile(&profile).await?.secrets()?);
            return manager
                .start(scope, root, command, input.interactive, env, bridge, wait)
                .await;
        }
        if input.command.is_some() || input.interactive {
            return Err("command/interactive only valid for start".into());
        }
        let id = input.session_id.ok_or("session_id required")?;
        match action {
            "read" if input.input.is_none() => manager.read(&scope, &id, input.after, wait).await,
            "write" => {
                manager
                    .write(&scope, &id, &input.input.ok_or("input required")?)
                    .await
            }
            "stop" if input.input.is_none() => manager.stop(&scope, &id).await,
            _ => Err("invalid shell action or arguments".into()),
        }
    }
}
