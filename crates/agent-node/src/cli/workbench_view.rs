use serde_json::Value;
pub(super) fn render(op: &str, value: &Value) -> String {
    let text = |row: &Value, key: &str| row[key].as_str().unwrap_or("—").to_owned();
    match op {
        "agents.list" => value["agents"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                format!(
                    "{} · {} · {} · {}{}",
                    text(r, "name"),
                    text(r, "id"),
                    text(r, "provider"),
                    if r["online"] == true {
                        "在线"
                    } else {
                        "已停止"
                    },
                    if r["kind"] == "remote" {
                        " · 远端只读"
                    } else {
                        ""
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "agents.show" => format!(
            "{}\nID：{}\n运行器：{}\n角色：{}\n版本：{}\n{}\n{}",
            text(value, "name"),
            text(value, "id"),
            text(value, "provider"),
            text(value, "role"),
            value["version"],
            if value["kind"] == "remote" {
                "远端只读：修改请在所属节点进行"
            } else {
                "本地 Agent"
            },
            value
                .get("policy")
                .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                .unwrap_or_default()
        ),
        "agents.save" => format!(
            "已保存 {} · {} · {} · 版本 {}",
            text(value, "client_id"),
            text(value, "provider"),
            text(value, "role"),
            value["version"]
        ),
        "projects.list" if value.as_array().is_some_and(|rows| rows.is_empty()) => {
            "暂无项目；/help projects 查看创建方式，或 /manage 让 Agent 帮你创建。".into()
        }
        "projects.list" => format!(
            "项目 · /chat ID或名称 进入 · /history 恢复记录\n{}",
            value
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| {
                    format!(
                        "{} · {} · {}",
                        text(&r["body"], "name"),
                        text(r, "key"),
                        match r["body"]["policy"]["mode"].as_str() {
                            Some("a2a" | "discussion") => "讨论模式",
                            Some("pmo" | "leader") => "Leader 模式",
                            Some("chat") => "简单聊天",
                            Some("relay") => "接力模式",
                            _ => "—",
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        ),
        "connections.list" => {
            if value.as_array().is_some_and(|r| r.is_empty()) {
                "尚无上游连接".into()
            } else {
                value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|r| {
                        format!(
                            "{} · {} · {} · {}",
                            text(r, "direction"),
                            text(r, "name"),
                            r["url"].as_str().or(r["node_id"].as_str()).unwrap_or("—"),
                            text(r, "status")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| "结果格式错误".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn project_modes_have_the_same_names_as_web() {
        let rows = json!([
            {"key":"one","body":{"name":"讨论","policy":{"mode":"a2a"}}},
            {"key":"two","body":{"name":"计划","policy":{"mode":"pmo"}}}
        ]);
        let text = render("projects.list", &rows);
        assert!(text.contains("讨论模式"));
        assert!(text.contains("Leader 模式"));
        assert!(!text.contains("a2a") && !text.contains("pmo"));
    }
}
