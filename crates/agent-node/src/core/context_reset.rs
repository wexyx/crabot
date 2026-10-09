use super::Core;
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    pub(crate) async fn reset_group_context(
        &self,
        project: Uuid,
        group: &str,
    ) -> Result<Value, String> {
        let _guard = self.lifecycle_lock().lock().await;
        self.project(project).await?;
        self.state()
            .policy_store
            .get(project, "group", group)
            .await?;
        if self.state().store.list("runs").await.iter().any(|r| {
            r["project_id"] == json!(project)
                && r["group_id"] == group
                && matches!(r["status"].as_str(), Some("queued" | "running"))
        }) {
            return Err("group is busy; interrupt it or wait before /new".into());
        }
        crate::storage::knowledge::persist(
            project,
            &format!("group:{group}"),
            &[json!({"type":"context.reset","content":"已开启新上下文，历史日志仍然保留。"})],
        )?;
        agent_runtime::workspace::revoke_conversation_approval(&format!("{project}:{group}"));
        agent_runtime::execution::ProcessSessions::global().stop_chat(&project.to_string(), group);
        Ok(json!({"message":"已开启新上下文，保留项目成员、策略和历史日志。"}))
    }
}

/// Reset markers live in the same append-only stream as messages, surviving restarts.
pub(crate) fn after_reset(rows: &[Value]) -> &[Value] {
    let start = rows
        .iter()
        .rposition(|r| r["type"] == "context.reset")
        .map_or(0, |i| i + 1);
    &rows[start..]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn last_reset_is_a_context_boundary_not_a_log_deletion() {
        let logs = vec![
            json!({"type":"user","content":"old"}),
            json!({"type":"context.reset"}),
            json!({"type":"user","content":"new"}),
        ];
        assert_eq!(after_reset(&logs).len(), 1);
        assert_eq!(after_reset(&logs)[0]["content"], "new");
        assert_eq!(logs.len(), 3);
    }
}
