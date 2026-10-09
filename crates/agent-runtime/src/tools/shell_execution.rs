use super::ToolContext;
use serde_json::Value;
pub(super) fn validate(command: &str) -> Result<(), String> {
    if command.trim().is_empty()
        || command.len() > 8192
        || command
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err("command must be 1..8192 bytes without terminal control characters".into());
    }
    Ok(())
}
pub(super) async fn run(context: &ToolContext, command: &str) -> Result<Value, String> {
    validate(command)?;
    let root = context.workdir().ok_or("workspace required")?.to_path_buf();
    let profile = context.execution.select_profile(None)?;
    crate::workspace::confirm_command(&root, command, &profile).await?;
    if root.canonicalize().map_err(|e| e.to_string())? != root {
        return Err("workspace changed during approval".into());
    }
    let bridge = crate::skills::SkillBridge::start(root.clone(), context.skills()).await?;
    let environment = bridge.as_ref().map(|b| b.environment()).unwrap_or_default();
    let result =
        crate::execution::execute_command(root, command.into(), profile, environment).await;
    drop(bridge);
    result
}
