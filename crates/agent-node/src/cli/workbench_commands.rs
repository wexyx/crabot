use super::commands::Command;
use serde_json::{Value, json};

fn split(value: &str) -> (&str, &str) {
    value
        .trim()
        .split_once(char::is_whitespace)
        .map(|(a, b)| (a, b.trim()))
        .unwrap_or((value.trim(), ""))
}
fn json_input(value: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(value).map_err(|e| format!("请输入 JSON 对象：{e}"))?;
    if !v.is_object() {
        return Err("参数必须是 JSON 对象".into());
    }
    Ok(v)
}
fn operation(op: &str, input: Value) -> Command {
    Command::Workbench(op.into(), input)
}
pub(super) fn parse(head: &str, tail: &str) -> Result<Option<Command>, String> {
    let (action, rest) = split(tail);
    Ok(Some(match head {
        "agents" => operation("agents.list", json!({})),
        "connect" => {
            if tail.trim().is_empty() {
                return Err("用法：/connect https://upstream.example".into());
            }
            operation("connections.connect", json!({"url":tail.trim()}))
        }
        "disconnect" | "reconnect" => {
            if !matches!(action, "upstream" | "downstream") || rest.is_empty() {
                return Err(
                    "用法：/disconnect upstream|downstream NAME（恢复用 /reconnect）".into(),
                );
            }
            operation(
                "connections.configure",
                json!({"direction":action,"name":rest,"disabled":head=="disconnect"}),
            )
        }
        "connections" => operation("connections.list", json!({})),
        "server" => match action {
            "" | "status" => operation("server.status", json!({})),
            "stop" => operation("server.stop", json!({})),
            "start" => operation(
                "server.start",
                if rest.is_empty() {
                    json!({})
                } else {
                    json!({"port":rest.parse::<u16>().map_err(|_|"用法：/server start [0..65535]")?})
                },
            ),
            _ => return Err("用法：/server status|start [PORT]|stop".into()),
        },
        "agent" => match action {
            "list" => operation("agents.list", json!({})),
            "show" | "start" | "stop" => {
                if rest.is_empty() {
                    return Err("需要 Agent ID".into());
                }
                operation(&format!("agents.{action}"), json!({"id":rest}))
            }
            "test" => {
                if rest.is_empty() {
                    return Err("用法：/agent test PATH".into());
                }
                operation(
                    "agents.test",
                    json!({"path":rest.split('/').collect::<Vec<_>>()}),
                )
            }
            "save" | "virtual" => operation(&format!("agents.{action}"), json_input(rest)?),
            "add" => {
                let (id, args) = split(rest);
                let (provider, role) = split(args);
                if id.is_empty() || provider.is_empty() || role.is_empty() {
                    return Err("用法：/agent add ID PROVIDER 角色".into());
                }
                operation(
                    "agents.save",
                    json!({"client_id":id,"provider":provider,"role":role,"expected_version":0}),
                )
            }
            "delete" => {
                let (id, args) = split(rest);
                let (version, confirm) = split(args);
                if confirm != "--confirm" {
                    return Err("删除前请 /agent show ID 查看版本；用法：/agent delete ID VERSION --confirm（保留历史）".into());
                }
                operation(
                    "agents.delete",
                    json!({"id":id,"expected_version":version.parse::<u64>().map_err(|_|"需要版本号")?}),
                )
            }
            "" => operation("agents.list", json!({})),
            _ => return Ok(None), // Existing /agent PATH role/start/... within a group.
        },
        "project" => match action {
            "" | "list" => operation("projects.list", json!({})),
            "create" => operation("projects.create", json_input(rest)?),
            "configure" => {
                let (id, body) = split(rest);
                let mut v = json_input(body)?;
                v["id"] = json!(id);
                operation("projects.configure", v)
            }
            _ => Command::Chat(tail.trim().into()),
        },
        "capabilities" => {
            if !matches!(action, "list" | "save" | "bind") {
                return Err("用法：/capabilities list|save|bind JSON；字段 scope、kind、agent/group、body 与 Web 一致".into());
            }
            operation(&format!("capabilities.{action}"), json_input(rest)?)
        }
        "skills" => {
            if !matches!(action, "list" | "save") {
                return Err("用法：/skills list|save JSON（scope、body）".into());
            }
            operation(&format!("skills.{action}"), json_input(rest)?)
        }
        "tool-library" => {
            if !matches!(
                action,
                "list" | "save" | "test" | "test-status" | "test-cancel"
            ) {
                return Err(
                    "用法：/tool-library list|save|test|test-status|test-cancel JSON".into(),
                );
            }
            operation(&format!("tools.{action}"), json_input(rest)?)
        }
        _ => return Ok(None),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_commands_are_unambiguous() {
        assert!(
            matches!(parse("project","").unwrap(),Some(Command::Workbench(op,_)) if op=="projects.list")
        );
        assert!(matches!(
            parse("project", "group-id").unwrap(),
            Some(Command::Chat(_))
        ));
        assert!(
            matches!(parse("agent","add coder codex 开发").unwrap(),Some(Command::Workbench(op,v)) if op=="agents.save"&&v["expected_version"]==0)
        );
        assert!(parse("agent", "coder role 开发").unwrap().is_none());
        assert!(parse("agent", "delete coder 1").is_err());
        assert!(parse("server", "start -1").is_err());
        assert!(parse("capabilities", "save []").is_err());
    }
}
