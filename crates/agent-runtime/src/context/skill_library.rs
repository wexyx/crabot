use super::HistoryFuture;
use serde_json::Value;
use std::{future::Future, sync::Arc};

pub trait SkillSource: Send + Sync {
    fn execute(&self, input: Value) -> HistoryFuture<'_>;
}
tokio::task_local! { static SOURCE: Arc<dyn SkillSource>; }
pub struct SkillAccess;
impl SkillAccess {
    pub async fn scope<F: Future>(source: Arc<dyn SkillSource>, future: F) -> F::Output {
        SOURCE.scope(source, future).await
    }
    pub async fn execute(input: Value) -> Result<Value, String> {
        SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "Skill library not bound to this run")?
            .execute(input)
            .await
    }
}
