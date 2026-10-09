use crate::management::Manager;
use serde_json::json;
use uuid::Uuid;

pub(super) async fn command(
    manager: &Manager,
    project: Uuid,
    chat: &str,
    args: &str,
) -> Result<(String, Option<String>), String> {
    let parts = args.split_whitespace().collect::<Vec<_>>();
    let op = parts.first().copied().unwrap_or("list");
    let id = parts.get(1).copied().unwrap_or("");
    if op == "list" && parts.len() <= 1 {
        let rows = manager
            .process_sessions(project, chat, "list", json!({}))
            .await?;
        let lines = rows
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| {
                format!(
                    "{} · {} · {} · {}",
                    v["session_id"].as_str().unwrap_or(""),
                    v["agent"].as_str().unwrap_or(""),
                    v["status"].as_str().unwrap_or(""),
                    v["command"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(60)
                        .collect::<String>()
                )
            })
            .collect::<Vec<_>>();
        return Ok((
            if lines.is_empty() {
                "当前聊天没有进程会话。".into()
            } else {
                format!(
                    "{}\n/process read ID 查看；/process input ID 私密输入；/process stop ID 停止",
                    lines.join("\n")
                )
            },
            None,
        ));
    }
    if id.is_empty() || parts.len() != 2 {
        return Err("用法：/process [list | read ID | input ID | stop ID]".into());
    }
    match op {
        "input" => {
            let row = manager
                .process_sessions(project, chat, "read", json!({"session_id":id}))
                .await?;
            if row["status"] != "running" {
                return Err("进程已结束".into());
            }
            Ok(("输入将直接发给进程，不进入聊天历史；Enter 发送，Esc 或 /cancel 取消。此后的进程输出仅用户可见。".into(),Some(id.into())))
        }
        "read" => {
            let v = manager
                .process_sessions(project, chat, "read", json!({"session_id":id}))
                .await?;
            let text = format!(
                "{}\n{}{}",
                v["status"].as_str().unwrap_or(""),
                v["stdout"].as_str().unwrap_or(""),
                v["stderr"].as_str().unwrap_or("")
            );
            Ok((
                text.chars()
                    .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
                    .collect(),
                None,
            ))
        }
        "stop" => {
            manager
                .process_sessions(project, chat, "stop", json!({"session_id":id}))
                .await?;
            Ok(("进程已停止。".into(), None))
        }
        _ => Err("用法：/process [list | read ID | input ID | stop ID]".into()),
    }
}
