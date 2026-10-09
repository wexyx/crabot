use super::Core;
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    pub(crate) async fn rename_chat(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        let id = input["id"].as_str().ok_or("id required")?;
        let name = input["name"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .ok_or("名称须为 1..256 字节")?;
        let mut row = self.state().policy_store.get(project, "group", id).await?;
        row.body["name"] = json!(name);
        row.body["auto_name"] = json!(false);
        Ok(json!(
            self.state()
                .policy_store
                .put(
                    project,
                    "group",
                    id,
                    input["expected_version"]
                        .as_u64()
                        .ok_or("expected_version required")?,
                    row.body
                )
                .await?
        ))
    }
    pub(crate) async fn delete_chat(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        let id = input["id"].as_str().ok_or("id required")?;
        let version = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let mut row = self.state().policy_store.get(project, "group", id).await?;
        if self.state().store.list("runs").await.iter().any(|r| {
            r["project_id"] == json!(project)
                && r["group_id"] == id
                && matches!(r["status"].as_str(), Some("running" | "queued"))
        }) {
            return Err("请先停止当前聊天任务，再删除聊天".into());
        }
        row.body["deleted"] = json!(true);
        self.state()
            .policy_store
            .put(project, "group", id, version, row.body)
            .await?;
        agent_runtime::execution::ProcessSessions::global().stop_chat(&project.to_string(), id);
        Ok(json!({"id":id,"deleted":true,"history_retained":true}))
    }
}
