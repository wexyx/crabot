use super::{Core, policies::Policy};
use crate::AppState;
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    pub(crate) async fn agent_directory(&self, project: Uuid) -> Result<Value, String> {
        self.project(project).await?;
        let local = self.state().store.list("local_agents").await;
        let inbound = self.state().store.list("peer_inbound").await;
        let clients = self.state().clients.lock().await;
        let mut entries = vec![];
        for row in local.iter().filter(|r| r["project_id"] == json!(project)) {
            let id = row["client_id"].as_str().unwrap_or_default();
            let online = clients
                .get(&(project, id.into()))
                .is_some_and(|c| !c.sender.is_closed());
            entries.push(json!({"id":id,"name":if row["is_default"]==true {"默认 Agent"}else{id},"kind":"local","response_instructions":row["response_instructions"].as_str().unwrap_or(super::response_instructions::DEFAULT),"permission_mode":row["permission_mode"].as_str().unwrap_or("ask"),"configuration":super::agent_configuration::AgentConfiguration::public(&row["configuration"]),"provider":row["provider"],"is_default":row["is_default"],"role":row["role"],"online":online,"version":row["version"].as_u64().unwrap_or(0)}));
        }
        for ((p, id), connection) in clients.iter() {
            if *p == project
                && connection
                    .node_id
                    .as_deref()
                    .is_some_and(|n| n != self.state().node_id)
                && !connection.sender.is_closed()
            {
                entries.push(json!({"id":id,"kind":"remote","provider":"crabot","role":connection.role,"online":true,"node_id":connection.node_id,"remote_address":inbound.iter().find(|r|r["project_id"]==json!(project)&&r["client_id"]==id.as_str()).map(|r|r["remote_address"].clone())}));
            }
        }
        drop(clients);
        for row in self
            .state()
            .policy_store
            .list(project, "virtual_agent")
            .await?
        {
            entries.push(json!({"id":row.key,"name":row.body["name"],"kind":"virtual","role":row.body.get("role").unwrap_or(&row.body["name"]),"online":true,"version":row.version,"response_instructions":row.body["response_instructions"].as_str().unwrap_or(super::response_instructions::DEFAULT),"policy":row.body["policy"]}));
        }
        entries.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let status = self.state().link_status.lock().await;
        let mounts = self.state().store.list("peer_mounts").await.into_iter()
            .filter(|r| r["local_project_id"] == json!(project))
            .map(|r| json!({"name":r["name"],"url":r["url"],"local_agents":r["local_agents"],"status":status.get(r["name"].as_str().unwrap_or_default()).cloned().unwrap_or("connecting".into())})).collect::<Vec<_>>();
        Ok(json!({"agents":entries,"mounts":mounts}))
    }
    pub(crate) async fn save_agent(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        if crate::web_config::proxy_mode() {
            return Err("proxy mode does not run local agents".into());
        }
        let id = input["client_id"].as_str().ok_or("client_id required")?;
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("Agent ID must be 1..64 letters, digits, _ or -".into());
        }
        let role = input["role"]
            .as_str()
            .filter(|r| !r.trim().is_empty() && r.len() <= 255)
            .ok_or("role required, maximum 255 bytes")?;
        let provider = input["provider"].as_str().ok_or("provider required")?;
        provider.parse::<agent_runtime::RuntimeKind>()?;
        if id == "default" {
            return self.save_default_role(project, input).await;
        }
        let expected = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let _guard = self.lifecycle_lock().lock().await;
        if self
            .state()
            .clients
            .lock()
            .await
            .contains_key(&(project, id.into()))
        {
            return Err("Stop the Agent before editing its configuration".into());
        }
        if self
            .state()
            .policy_store
            .get(project, "virtual_agent", id)
            .await
            .is_ok()
        {
            return Err("Agent ID already exists".into());
        }
        let key = format!("{project}:{id}");
        self.state().store.transaction(|data| {
            let old = data.get("local_agents", &key).cloned();
            if old.as_ref().and_then(|r|r["version"].as_u64()).unwrap_or(0)!=expected { return Err("version_conflict".into()); }
            if old.is_none() {
                data.credential(json!({"ak":format!("local_{}",Uuid::new_v4().simple()),"sk_hash":crate::hash_secret(&Uuid::new_v4().to_string()),"project_id":project,"client_id":id,"role":role}))?;
            }
            let configuration=super::agent_configuration::AgentConfiguration::merge(&old.as_ref().map(|r|r["configuration"].clone()).unwrap_or(Value::Null),input.get("configuration"))?;
            let permission=old.as_ref().and_then(|r|r["permission_mode"].as_str()).unwrap_or("ask");
            let response_instructions=super::response_instructions::resolve(&input,old.as_ref())?;
            let row=json!({"response_instructions":response_instructions,"permission_mode":permission,"configuration":configuration,"project_id":project,"client_id":id,"role":role,"provider":provider,"enabled":false,"version":expected.checked_add(1).ok_or("version overflow")?});
            data.set("local_agents",&key,row.clone());
            Ok(super::agent_configuration::AgentConfiguration::redact(row))
        }).await
    }
}

// Applies to UI, CLI and management tools, not just the selection widget.
pub(super) async fn validate_members(
    state: &AppState,
    project: Uuid,
    policy: &Policy,
    previous: Option<&Policy>,
) -> Result<(), String> {
    let virtuals = state.policy_store.list(project, "virtual_agent").await?;
    let locals = state.store.list("local_agents").await;
    for member in &policy.members {
        if previous.is_some_and(|p| p.members.iter().any(|m| m.path == member.path)) {
            continue;
        }
        if member.path.len() == 2 {
            let rows = super::remote_agents::catalog(state, project, &member.path[0]).await?;
            if rows.iter().any(|r| r["id"] == member.path[1]) {
                continue;
            }
            return Err("unknown remote Agent".into());
        }
        if member.path.len() != 1 {
            return Err("Select a configured local Agent or directly connected Crabot; nested paths cannot be added as project members".into());
        }
        let id = &member.path[0];
        let local = locals
            .iter()
            .any(|r| r["project_id"] == json!(project) && r["client_id"] == id.as_str());
        let remote = state
            .clients
            .lock()
            .await
            .get(&(project, id.clone()))
            .is_some_and(|c| {
                !c.sender.is_closed() && c.node_id.as_deref().is_some_and(|n| n != state.node_id)
            });
        let virtual_agent = virtuals.iter().any(|v| v.key == *id);
        if !local && !remote && !virtual_agent {
            return Err(format!(
                "Unknown member {id}: configure it in Agent management or connect the remote Crabot first"
            ));
        }
    }
    Ok(())
}
