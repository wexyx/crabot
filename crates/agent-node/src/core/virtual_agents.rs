use super::{
    Core,
    policies::{Policy, engine},
};
use crate::AppState;
use serde_json::{Value, json};
use uuid::Uuid;
tokio::task_local! { static STACK: Vec<String>; }

impl Core {
    pub(crate) async fn save_virtual_agent(&self, p: Uuid, input: Value) -> Result<Value, String> {
        self.project(p).await?;
        let policy: Policy =
            serde_json::from_value(input["policy"].clone()).map_err(|e| e.to_string())?;
        policy.validate()?;
        if policy.mode == super::policies::Mode::Chat {
            return Err("virtual Agents require relay, a2a or pmo composition".into());
        }
        let name = input["name"]
            .as_str()
            .filter(|n| !n.trim().is_empty() && n.len() <= 256)
            .ok_or("virtual Agent name required")?;
        let role = input["role"]
            .as_str()
            .filter(|role| !role.trim().is_empty())
            .unwrap_or(name);
        if role.len() > 255 {
            return Err("role maximum 255 bytes".into());
        }
        let id = input["id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("virtual-{}", Uuid::new_v4()));
        if input["id"].is_string()
            && self
                .state()
                .policy_store
                .get(p, "virtual_agent", &id)
                .await
                .is_err()
        {
            return Err("unknown virtual Agent".into());
        }
        let expected = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let _guard = self.lifecycle_lock().lock().await;
        if self
            .state()
            .store
            .get("local_agents", &format!("{p}:{id}"))
            .await
            .is_some()
        {
            return Err("Agent ID already exists".into());
        }
        super::agent_directory::validate_members(self.state(), p, &policy, None).await?;
        let rows = self.state().policy_store.list(p, "virtual_agent").await?;
        fn visit(
            id: &str,
            policy: &Policy,
            rows: &[crate::policy_store::Document],
            stack: &mut Vec<String>,
        ) -> Result<(), String> {
            if stack.len() >= 8 || stack.iter().any(|s| s == id) {
                return Err("virtual Agent cycle or nesting exceeds 8 levels".into());
            }
            stack.push(id.into());
            for m in &policy.members {
                if stack.contains(&m.path[0]) {
                    return Err("virtual Agent cycle".into());
                }
                if let Some(row) = rows.iter().find(|r| r.key == m.path[0]) {
                    let child = serde_json::from_value(row.body["policy"].clone())
                        .map_err(|e| format!("{e}"))?;
                    visit(&row.key, &child, rows, stack)?;
                }
            }
            stack.pop();
            Ok(())
        }
        visit(&id, &policy, &rows, &mut vec![])?;
        let response_instructions = super::response_instructions::resolve(
            &input,
            rows.iter().find(|r| r.key == id).map(|r| &r.body),
        )?;
        Ok(json!(
            self.state()
                .policy_store
                .put(
                    p,
                    "virtual_agent",
                    &id,
                    expected,
                    json!({"name":name,"role":role,"policy":policy,"response_instructions":response_instructions})
                )
                .await?
        ))
    }
}

pub(super) async fn execute(
    state: &AppState,
    p: Uuid,
    id: &str,
    policy: Policy,
    prompt: &str,
    visited: &[String],
) -> Result<String, String> {
    let mut stack = STACK.try_with(Clone::clone).unwrap_or_default();
    if stack.len() >= 8 || stack.iter().any(|s| s == id) {
        return Err("virtual Agent recursion blocked".into());
    }
    stack.push(id.into());
    let config = state.policy_store.get(p, "virtual_agent", id).await?;
    let prompt = format!(
        "Response requirements: {}\n{prompt}",
        super::response_instructions::current(config.body["response_instructions"].as_str())?
    );
    STACK
        .scope(stack, invoke(state, p, policy, &prompt, visited))
        .await
}

/// Invoke an ephemeral composition without looking up a persisted virtual Agent.
pub(super) async fn invoke(
    state: &AppState,
    p: Uuid,
    policy: Policy,
    prompt: &str,
    visited: &[String],
) -> Result<String, String> {
    // Internal tool/progress messages stay inside; the caller sees one answer.
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    let dispatch = super::policies::Dispatch::new(&tx, visited);
    let work = Box::pin(engine(state, p, &policy, prompt, &dispatch));
    tokio::pin!(work);
    loop {
        tokio::select! {
            result=&mut work=>return result,
            _=rx.recv()=>{}
        }
    }
}

pub(super) async fn local_only(state: &AppState, p: Uuid, policy: &Policy, depth: usize) -> bool {
    if depth >= 8 {
        return false;
    }
    for member in &policy.members {
        if member.path.len() != 1 {
            return false;
        }
        if let Ok(agent) = state
            .policy_store
            .get(p, "virtual_agent", &member.path[0])
            .await
        {
            let Ok(child) = serde_json::from_value(agent.body["policy"].clone()) else {
                return false;
            };
            if !Box::pin(local_only(state, p, &child, depth + 1)).await {
                return false;
            }
        } else if state
            .store
            .get("local_agents", &format!("{p}:{}", member.path[0]))
            .await
            .is_none()
        {
            return false;
        }
    }
    true
}
