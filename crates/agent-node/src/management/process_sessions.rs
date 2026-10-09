use super::Manager;
use serde_json::{Value, json};
use uuid::Uuid;

impl Manager {
    /// Human-only boundary shared by Web and CLI, not registered as a model tool.
    pub(crate) async fn process_sessions(
        &self,
        project: Uuid,
        chat: &str,
        op: &str,
        input: Value,
    ) -> Result<Value, String> {
        self.core().project(project).await?;
        if chat != "admin" {
            self.core()
                .state()
                .policy_store
                .get(project, "group", chat)
                .await?;
        }
        let manager = agent_runtime::execution::ProcessSessions::global();
        let p = project.to_string();
        if op == "list" {
            return Ok(manager.list(&p, chat));
        }
        let id = input["session_id"].as_str().ok_or("session_id required")?;
        match op {
            "read" => manager.human_read(&p, chat, id, input["after"].as_u64().unwrap_or(0)),
            "write" => {
                manager
                    .human_write(
                        &p,
                        chat,
                        id,
                        input["input"].as_str().ok_or("input required")?,
                        input["private"].as_bool().unwrap_or(true),
                    )
                    .await
            }
            "stop" => {
                manager.human_stop(&p, chat, id).await?;
                Ok(json!({"session_id":id,"stopped":true}))
            }
            _ => Err("unknown process operation".into()),
        }
    }
}
