use super::{Manager, session};
use serde_json::{Value, json};
use uuid::Uuid;

impl Manager {
    pub(crate) async fn reset_context(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        let active = self.active_sessions().lock().await;
        if active.contains_key(&id) {
            return Err("management session busy; interrupt or wait before /new".into());
        }
        self.history(project, id).await?;
        session::append(
            self.core(),
            id,
            json!({"type":"context.reset","content":"已开启新上下文，历史日志仍然保留。"}),
        )
        .await?;
        agent_runtime::execution::ProcessSessions::global()
            .stop_chat(&project.to_string(), "admin");
        Ok(json!({"message":"已开启新上下文，历史日志仍然保留。"}))
    }
}
