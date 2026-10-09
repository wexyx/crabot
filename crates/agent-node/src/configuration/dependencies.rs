pub(super) fn check(key: &str, command: &str) -> Result<(), String> {
    let (name, url) = match key {
        "CODEX_BIN" => ("Codex", "https://learn.chatgpt.com/docs/codex/cli"),
        "CLAUDE_BIN" => ("Claude Code", "https://code.claude.com/docs/en/setup"),
        "OPENCODE_BIN" => ("OpenCode", "https://opencode.ai/docs/#install"),
        _ => return Ok(()),
    };
    agent_runtime::config::LauncherAvailability::check(command).map(|_|()).map_err(|error|format!("{error}\n请先安装 {name}：{url}\n在另一个终端安装并完成登录后，回车重新检测；也可以填写可执行文件的绝对路径，或 Esc 取消。"))
}
