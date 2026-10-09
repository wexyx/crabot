use super::{
    Core,
    policies::{Member, Mode, Policy},
};
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) const HELP: &str = "@名字 内容（只与该 Agent 对话；未 @ 时按群模式协作）\n/new（新上下文，保留历史）\n/members（或 /agents）\n/add-agent PATH [群内角色]\n/remove-agent PATH\n/agent PATH\n/agent PATH role 群内角色\n/agent PATH leader\n/agent PATH provider mock|crabot|codex|claude|opencode（先停止）\n/agent PATH start\n/group mode chat|relay|discussion|leader\n/group instructions 群协作要求";
impl Core {
    pub(crate) async fn group_command(
        &self,
        project: Uuid,
        key: &str,
        line: &str,
    ) -> Result<Value, String> {
        if line.trim() == "/new" {
            return self.reset_group_context(project, key).await;
        }
        self.project(project).await?;
        let document = self.state().policy_store.get(project, "group", key).await?;
        let mut policy: Policy =
            serde_json::from_value(document.body["policy"].clone()).map_err(|e| e.to_string())?;
        let (command, args) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        if command == "/help" {
            return Ok(json!({"message":HELP}));
        }
        if matches!(command, "/agents" | "/members") {
            return Ok(
                json!({"message":policy.members.iter().map(|m|format!("{} · {}{}",m.path.join("/"),m.role,if policy.leader.as_ref()==Some(&m.path){" · leader"}else{""})).collect::<Vec<_>>().join("\n"),"members":policy.members,"version":document.version}),
            );
        }
        let (target, rest) = args.trim().split_once(' ').unwrap_or((args.trim(), ""));
        let path = target.split('/').map(str::to_owned).collect::<Vec<_>>();
        match command {
            "/add-agent" => {
                if target.is_empty() {
                    return Err("usage: /add-agent PATH [role]".into());
                }
                policy.members.push(Member {
                    path,
                    role: if rest.trim().is_empty() {
                        self.agent_candidates(project).await?["agents"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|a| a["id"] == target)
                            .and_then(|a| a["role"].as_str())
                            .unwrap_or("member")
                            .to_owned()
                    } else {
                        rest.trim().into()
                    },
                });
            }
            "/remove-agent" => {
                let before = policy.members.len();
                policy.members.retain(|m| m.path != path);
                if before == policy.members.len() {
                    return Err("Agent is not a member of this group".into());
                }
                if policy.leader.as_ref() == Some(&path) {
                    return Err(
                        "先用 /agent PATH leader 指定另一个 Leader，再移除当前 Leader".into(),
                    );
                }
            }
            "/agent" => {
                let member = policy
                    .members
                    .iter_mut()
                    .find(|m| m.path == path)
                    .ok_or("Agent is not a member of this group")?;
                let (field, value) = rest.trim().split_once(' ').unwrap_or((rest.trim(), ""));
                match field {
                    "" => {
                        let local = if path.len() == 1 {
                            self.state()
                                .store
                                .get("local_agents", &format!("{project}:{}", path[0]))
                                .await
                        } else {
                            None
                        };
                        return Ok(
                            json!({"message":format!("{}\n群内角色：{}\nProvider：{}",target,member.role,local.as_ref().and_then(|r|r["provider"].as_str()).unwrap_or("remote / unknown")),"member":member,"provider":local.as_ref().map(|r|r["provider"].clone())}),
                        );
                    }
                    "role" => {
                        if value.trim().is_empty() {
                            return Err("role must not be empty".into());
                        }
                        member.role = value.trim().into();
                    }
                    "leader" => policy.leader = Some(path),
                    "provider" => {
                        if path.len() != 1 {
                            return Err(
                                "远端 Agent 运行器由其所属节点配置；这里可调整群内角色".into()
                            );
                        }
                        return self
                            .configure_stopped_agent(project, &path[0], value.trim())
                            .await;
                    }
                    "start" => {
                        if path.len() != 1 {
                            return Err("启动远端 Agent 请在其所属节点操作".into());
                        }
                        let row = self
                            .state()
                            .store
                            .get("local_agents", &format!("{project}:{}", path[0]))
                            .await
                            .ok_or("not a local agent")?;
                        self.agent_start(project, row).await?;
                        return Ok(json!({"message":format!("已启动 {target}")}));
                    }
                    _ => return Err(HELP.into()),
                }
            }
            "/group" => match target {
                "mode" => {
                    policy.mode = match rest.trim() {
                        "chat" => Mode::Chat,
                        "relay" => Mode::Relay,
                        "discussion" | "a2a" | "讨论" => Mode::A2a,
                        "leader" | "pmo" => Mode::Pmo,
                        _ => return Err("mode must be chat, relay, discussion or leader".into()),
                    }
                }
                "instructions" => policy.instructions = rest.trim().into(),
                _ => return Err(HELP.into()),
            },
            _ => return Err(HELP.into()),
        }
        policy.validate()?;
        let updated = self
            .control(
                project,
                "group.update",
                json!({"key":key,"expected_version":document.version,"policy":policy}),
            )
            .await?;
        let message = format!(
            "群配置已更新（版本 {}）。当前执行仍使用原策略，新一轮使用新配置。",
            updated["version"]
        );
        crate::storage::knowledge::persist(
            project,
            &format!("group:{key}"),
            &[json!({"type":"command.result","content":message,"command":line})],
        )?;
        Ok(json!({"message":message,"group":updated}))
    }
}
