pub(super) enum Command {
    Help(String),
    Workbench(String, serde_json::Value),
    History(String),
    Tools(Option<usize>),
    Exit,
    Update,
    Back,
    New,
    Resume(String),
    Chat(String),
    Admin,
    AdminConfig,
    Permissions(String),
    Allowlist(String),
    Prompts(String),
    Interrupt,
    Confirm(String, bool),
    WorkspaceConfirm(String, bool),
    Say(String),
    Group(String),
    Attach(String),
    Detach,
}
pub(super) fn parse(line: &str) -> Result<Command, String> {
    let line = line.trim();
    if !line.starts_with('/') {
        return Ok(Command::Say(line.into()));
    }
    let (head, tail) = line[1..]
        .split_once(char::is_whitespace)
        .unwrap_or((&line[1..], ""));
    let tail = tail.trim();
    if head == "namespace" {
        return Err("/namespace 已废弃；使用 /chat 查看项目，或 /manage 进入管理。".into());
    }
    let head = super::command_catalog::canonical(head).unwrap_or(head);
    if let Some(entry) = super::command_catalog::entry(head) {
        if !entry.arguments && !tail.is_empty() {
            return Err(format!("用法：{}", entry.name));
        }
    }
    if let Some(command) = super::workbench_commands::parse(head, tail)? {
        return Ok(command);
    }
    Ok(match head {
        "attach" => Command::Attach(tail.trim().into()),
        "detach" => Command::Detach,
        "add-agent" | "remove-agent" | "agent" | "group" => {
            Command::Group(format!("/{head} {tail}"))
        }
        "members" => Command::Group("/agents".into()),
        "manage" => Command::Admin,
        "back" => Command::Back,
        "history" => Command::History(tail.trim().into()),
        "tools" => Command::Tools(if tail.trim().is_empty() {
            None
        } else {
            Some(tail.trim().parse().map_err(|_| "usage: /tools [number]")?)
        }),
        "" | "help" => Command::Help(tail.trim().into()),
        "exit" => Command::Exit,
        "update" if tail.trim().is_empty() => Command::Update,
        "update" => return Err("用法：/update".into()),
        "allowlist" => Command::Allowlist(tail.trim().into()),
        "prompts" => Command::Prompts(tail.trim().into()),
        "permissions" => Command::Permissions(tail.trim().into()),
        "new" => Command::New,
        "resume" => Command::Resume(tail.trim().into()),
        "chat" if tail.is_empty() => {
            Command::Workbench("projects.list".into(), serde_json::json!({}))
        }
        "chat" => Command::Chat(tail.into()),
        "admin" => Command::Admin,
        "agent-config" | "admin-config" => Command::AdminConfig,
        "interrupt" => Command::Interrupt,
        "approve" | "deny" => Command::Confirm(tail.trim().into(), head == "approve"),
        "allow-path" | "deny-path" => {
            Command::WorkspaceConfirm(tail.trim().into(), head == "allow-path")
        }
        _ => {
            return Err(format!(
                "未知命令 /{head}；输入 / 查看命令菜单，或 /help 查看帮助。"
            ));
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_aliases_whitespace_and_invalid_arguments() {
        for input in ["/chat", "/chat  ", "/sessions", "/project"] {
            assert!(
                matches!(parse(input).unwrap(), Command::Workbench(op,_) if op=="projects.list")
            );
        }
        assert!(
            matches!(parse(" /chat\tmy project ").unwrap(),Command::Chat(id) if id=="my project")
        );
        assert!(matches!(parse("/back").unwrap(), Command::Back));
        assert!(matches!(parse("/admin").unwrap(), Command::Admin));
        assert!(matches!(parse("/quit").unwrap(), Command::Exit));
        for input in [
            "/new unexpected",
            "/exit oops",
            "/back wrong",
            "/agents extra",
            "/namespace",
            "/unknown",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
        assert!(
            matches!(parse("/agents").unwrap(),Command::Workbench(op,v) if op=="agents.list" && v.get("contextual").is_none())
        );
        assert!(matches!(parse("/members").unwrap(), Command::Group(_)));
    }
    #[test]
    fn all_catalog_entries_have_a_parser_route() {
        for entry in super::super::command_catalog::ENTRIES {
            if let Err(error) = parse(entry.name) {
                assert!(!error.contains("未知命令"), "{}: {error}", entry.name);
            }
        }
    }
    #[test]
    fn management_is_natural_language_not_a_command() {
        assert!(matches!(parse("web start").unwrap(), Command::Say(_)));
        assert!(matches!(parse("帮我建群").unwrap(), Command::Say(_)));
        assert!(parse("/tool agent_start {}").is_err());
        assert!(matches!(
            parse("/approve abc").unwrap(),
            Command::Confirm(_, true)
        ));
    }
}
