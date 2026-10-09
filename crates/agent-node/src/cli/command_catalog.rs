/// Shared by help, completion and argument validation. Aliases are accepted but
/// only suggested when explicitly typed, keeping the primary menu compact.
pub(super) struct CommandEntry {
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) arguments: bool,
}
impl CommandEntry {
    const fn new(name: &'static str, description: &'static str, arguments: bool) -> Self {
        Self {
            name,
            description,
            arguments,
        }
    }
}
pub(super) const ENTRIES: &[CommandEntry] = &[
    CommandEntry::new("/help", "帮助与命令说明", true),
    CommandEntry::new("/chat", "选择聊天 · 新建 / 进入已有聊天", true),
    CommandEntry::new("/history", "恢复当前聊天记录 · 可指定项目", true),
    CommandEntry::new("/new", "重置当前上下文，保留记录", false),
    CommandEntry::new("/manage", "进入管理对话", false),
    CommandEntry::new("/back", "返回上一个聊天", false),
    CommandEntry::new("/agents", "所有 Agent · 本地与远端", false),
    CommandEntry::new("/members", "当前项目成员", false),
    CommandEntry::new("/tools", "查看工具调用 · 可指定序号", true),
    CommandEntry::new("/attach", "附加文件 · 当前机器路径", true),
    CommandEntry::new("/detach", "清空待发送附件", false),
    CommandEntry::new("/permissions", "查看 / 切换执行权限", true),
    CommandEntry::new("/agent-config", "配置默认 Agent", false),
    CommandEntry::new("/agent", "Agent 增删改查 / 测试", true),
    CommandEntry::new("/project", "项目列表 / 创建 / 配置", true),
    CommandEntry::new("/add-agent", "添加项目成员", true),
    CommandEntry::new("/remove-agent", "移除项目成员", true),
    CommandEntry::new("/group", "项目协作模式与共同要求", true),
    CommandEntry::new("/prompts", "系统提示词 · 查看 / 覆盖 / 恢复默认", true),
    CommandEntry::new("/allowlist", "自动批准命令白名单", true),
    CommandEntry::new("/server", "Server 状态 / 启动 / 停止", true),
    CommandEntry::new("/connections", "已接入网络", false),
    CommandEntry::new("/connect", "申请连接上游 Crabot", true),
    CommandEntry::new("/disconnect", "断开连接", true),
    CommandEntry::new("/reconnect", "手动恢复连接", true),
    CommandEntry::new("/capabilities", "能力库与启用范围 · JSON", true),
    CommandEntry::new("/skills", "Skill 库 · JSON", true),
    CommandEntry::new("/tool-library", "工具配置与测试 · JSON", true),
    CommandEntry::new("/approve", "批准管理请求 · 请求 ID", true),
    CommandEntry::new("/deny", "拒绝管理请求 · 请求 ID", true),
    CommandEntry::new("/allow-path", "批准执行请求 · 请求 ID", true),
    CommandEntry::new("/deny-path", "拒绝执行请求 · 请求 ID", true),
    CommandEntry::new("/update", "下载更新，重启生效", false),
    CommandEntry::new("/interrupt", "打断当前任务", false),
    CommandEntry::new("/exit", "退出 Crabot", false),
];
const ALIASES: &[(&str, &str)] = &[
    ("admin", "manage"),
    ("admin-config", "agent-config"),
    ("sessions", "chat"),
    ("quit", "exit"),
    ("q", "exit"),
];
pub(super) fn canonical(name: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(_, name)| *name)
}
pub(super) fn entry(name: &str) -> Option<&'static CommandEntry> {
    ENTRIES.iter().find(|entry| entry.name[1..] == *name)
}
pub(super) fn suggestions(prefix: &str) -> Vec<&'static str> {
    let mut names: Vec<_> = ENTRIES
        .iter()
        .filter(|entry| entry.name.starts_with(prefix))
        .map(|entry| entry.name)
        .collect();
    if prefix.len() > 1 {
        for alias in [
            "/admin",
            "/admin-config",
            "/sessions",
            "/quit",
            "/q",
            "/resume",
        ] {
            if alias.starts_with(prefix) {
                names.push(alias);
            }
        }
    }
    names
}
pub(super) fn description(name: &str) -> &'static str {
    let name = name.trim_start_matches('/');
    entry(canonical(name).unwrap_or(name))
        .map(|entry| entry.description)
        .unwrap_or("恢复管理会话 · SESSION_ID")
}
