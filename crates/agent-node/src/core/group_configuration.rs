use super::{Core, policies::Policy};
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    /// One versioned edit for the complete policy, including dependent mode/leader fields.
    pub(crate) async fn configure_group(
        &self,
        project: Uuid,
        key: &str,
        input: Value,
    ) -> Result<Value, String> {
        self.project(project).await?;
        let policy: Policy =
            serde_json::from_value(input["policy"].clone()).map_err(|e| e.to_string())?;
        policy.validate()?;
        let mut update = input.clone();
        update["key"] = json!(key);
        update["policy"] = json!(policy);
        let updated = self.control(project, "group.update", update).await?;
        crate::storage::knowledge::persist(
            project,
            &format!("group:{key}"),
            &[
                json!({"type":"command.result","content":"群配置已更新；后续轮次使用新策略。","version":updated["version"]}),
            ],
        )?;
        Ok(updated)
    }
}
