use crate::core::Core;
use serde_json::{Value, json};
use uuid::Uuid;
pub(super) async fn append(core: &Core, id: Uuid, event: Value) -> Result<Value, String> {
    Ok(append_batch(core, id, vec![event], None).await?.remove(0))
}
pub(super) async fn append_batch(
    core: &Core,
    id: Uuid,
    batch: Vec<Value>,
    status: Option<&str>,
) -> Result<Vec<Value>, String> {
    let row = core
        .state()
        .store
        .get("management_sessions", &id.to_string())
        .await
        .ok_or("unknown management session")?;
    let project = Uuid::parse_str(
        row["project_id"]
            .as_str()
            .ok_or("session missing project")?,
    )
    .map_err(|e| e.to_string())?;
    let events = crate::storage::knowledge::persist(project, "admin", &batch)?;
    // Chat events are committed first. Metadata is only a small materialized index.
    if let Some(status) = status {
        let mut row = row;
        row["status"] = json!(status);
        row["updated_at"] = json!(crate::storage::now());
        core.state()
            .store
            .transaction(|d| {
                d.set("management_sessions", &id.to_string(), row);
                Ok(())
            })
            .await?;
    }
    Ok(events)
}
