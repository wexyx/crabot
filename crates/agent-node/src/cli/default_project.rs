use crate::management::Manager;
use serde_json::json;
use uuid::Uuid;

/// A stable ordinary project, not an administrative or hidden test session.
pub(super) async fn ensure(manager: &Manager, project: Uuid) -> Result<String, String> {
    let state = manager.core().state();
    let key = project.to_string();
    if let Some(saved) = state.store.get("cli_defaults", &key).await {
        if let Some(id) = saved["group"].as_str() {
            if let Ok(group) = state.policy_store.get(project, "group", id).await {
                let policy = &group.body["policy"];
                if policy["mode"] == "chat"
                    && policy["members"].as_array().is_some_and(|members| {
                        members.len() == 1 && members[0]["path"] == json!(["default"])
                    })
                {
                    return Ok(id.into());
                }
            }
        }
    }
    let id = create(manager, project).await?;
    state
        .store
        .transaction(|data| {
            data.set("cli_defaults", &key, json!({"group":id}));
            Ok(())
        })
        .await?;
    Ok(id)
}

/// A fresh simple chat; unlike ensure, this never reuses the startup project.
pub(super) async fn create(manager: &Manager, project: Uuid) -> Result<String, String> {
    let default = manager
        .core()
        .state()
        .store
        .get("local_agents", &format!("{project}:default"))
        .await;
    let role = default
        .as_ref()
        .and_then(|r| r["role"].as_str())
        .unwrap_or("默认助手");
    let group = manager.core().control(project, "group.create", json!({
        "name": "",
        "policy": {"mode":"chat","members":[{"path":["default"],"role":role}],"leader":null,"rounds":1,"instructions":""}
    })).await?;
    let id = group["key"]
        .as_str()
        .ok_or("default chat missing key")?
        .to_owned();
    Ok(id)
}
