//! Local children execute inside this Agent process, without loopback HTTP or AK/SK.
use crate::*;
use agent_runtime::{RuntimeEvent, RuntimeFactory, RuntimeKind, is_token_insufficient};

#[derive(Serialize, Deserialize)]
pub struct Child {
    client_id: String,
    role: String,
    provider: String,
}

pub async fn initialize(state: &AppState) {
    if web_config::proxy_mode() {
        return;
    }
    for row in state.store.list("local_agents").await {
        if row["enabled"] == false || row["is_default"] == true {
            continue;
        }
        start(
            state.clone(),
            Uuid::parse_str(&storage::field(&row, "project_id")).expect("project ID"),
            serde_json::from_value(row).expect("local agent configuration"),
        )
        .await;
    }
}

pub(crate) async fn register(
    state: &AppState,
    project: Uuid,
    child: Child,
) -> Result<Value, String> {
    if web_config::proxy_mode() {
        return Err("proxy mode does not run local agents".into());
    }
    if child.client_id.trim().is_empty()
        || child.client_id.len() > 255
        || child.role.len() > 255
        || child.provider.parse::<RuntimeKind>().is_err()
    {
        return Err("invalid agent definition".into());
    }
    if state
        .store
        .get("projects", &project.to_string())
        .await
        .is_none()
    {
        return Err("unknown project".into());
    }
    if state
        .policy_store
        .get(project, "virtual_agent", &child.client_id)
        .await
        .is_ok()
    {
        return Err("Agent ID already exists".into());
    }
    state.store.transaction(|data| {
        data.credential(json!({"ak":format!("local_{}",Uuid::new_v4().simple()),"sk_hash":hash_secret(&Uuid::new_v4().to_string()),"project_id":project,"client_id":child.client_id,"role":child.role}))?;
        data.insert("local_agents",&format!("{project}:{}",child.client_id),json!({"project_id":project,"client_id":child.client_id,"role":child.role,"provider":child.provider}))
    }).await?;
    let result = json!({"client_id":child.client_id,"project_id":project,"role":child.role,"provider":child.provider,"kind":"local"});
    start(state.clone(), project, child).await;
    Ok(result)
}

pub(crate) async fn start(state: AppState, project_id: Uuid, child: Child) {
    start_configured(state, project_id, child, None).await;
}
pub(crate) async fn start_configured(
    state: AppState,
    project_id: Uuid,
    child: Child,
    config: Option<agent_runtime::config::RuntimeConfig>,
) {
    let credential = Credential {
        project_id,
        client_id: child.client_id,
        role: child.role,
    };
    let (tx, mut rx) = mpsc::channel::<WireEvent>(32);
    let generation = tx.downgrade();
    state.clients.lock().await.insert(
        (project_id, credential.client_id.clone()),
        ClientConnection {
            sender: tx,
            role: credential.role.clone(),
            node_id: Some(state.node_id.clone()),
        },
    );
    tokio::spawn(async move {
        // One task at a time per local child; independent children can run concurrently.
        while let Some(command) = rx.recv().await {
            if command.kind == "command" {
                let enabled = state
                    .store
                    .get(
                        "local_agents",
                        &format!("{project_id}:{}", credential.client_id),
                    )
                    .await
                    .is_none_or(|r| r["enabled"] != false);
                let current = state
                    .clients
                    .lock()
                    .await
                    .get(&(project_id, credential.client_id.clone()))
                    .is_some_and(|c| {
                        generation
                            .upgrade()
                            .is_some_and(|sender| sender.same_channel(&c.sender))
                    });
                if !enabled || !current {
                    emit(
                        &state,
                        &credential,
                        &command,
                        "agent.error",
                        "Agent 已停用，不接收新任务",
                    )
                    .await;
                    continue;
                }
                execute(
                    &state,
                    &credential,
                    &child.provider,
                    &command,
                    config.as_ref(),
                )
                .await;
            }
        }
    });
}

async fn emit(
    state: &AppState,
    credential: &Credential,
    command: &WireEvent,
    kind: &str,
    content: &str,
) {
    let token_limit = kind == "agent.error" && is_token_insufficient(content);

    let Ok(message_id) = serde_json::from_value::<Uuid>(command.data["message_id"].clone()) else {
        return;
    };
    let _ = accept_client_event(
        state.clone(),
        credential.clone(),
        ClientEvent {
            session_id: command.session_id,
            message_id,
            kind: kind.into(),
            content: Some(content.into()),
            error: None,
            error_code: token_limit.then(|| "token_insufficient".into()),
        },
    )
    .await;
}

async fn emit_progress(
    state: &AppState,
    credential: &Credential,
    command: &WireEvent,
    event: &RuntimeEvent,
) {
    if let Some((kind, content)) = event.wire_progress() {
        emit(state, credential, command, kind, &content).await;
    }
}

pub(super) async fn execute(
    state: &AppState,
    credential: &Credential,
    provider: &str,
    command: &WireEvent,
    configured: Option<&agent_runtime::config::RuntimeConfig>,
) {
    let message_id = serde_json::from_value::<Uuid>(command.data["message_id"].clone()).ok();
    if let Some(id) = message_id {
        if conversation::stopped(state, id).await {
            return;
        }
    }
    let chat = command.data["capability_project"]
        .as_str()
        .map(|g| format!("group:{g}"))
        .unwrap_or_else(|| format!("session:{}", command.session_id));
    let source = std::sync::Arc::new(super::indexed_history::IndexedHistory::new(
        credential.project_id,
        chat,
        Some(credential.client_id.clone()),
    ));
    let before = command.data["history_before"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| message_id.map(|id| id.to_string()));
    let history = match source.context_prompt(before).await {
        Ok(history) => {
            if command.data["capability_project"].is_string() {
                String::new()
            } else {
                history
            }
        }
        Err(error) => {
            emit(state, credential, command, "agent.error", &error).await;
            return;
        }
    };
    let definition = state
        .store
        .get(
            "local_agents",
            &format!("{}:{}", credential.project_id, credential.client_id),
        )
        .await;
    let role = definition
        .as_ref()
        .and_then(|row| row["role"].as_str())
        .unwrap_or(&credential.role);
    let prompt = format!(
        "Your registered role: {}\nTask: {}\nLatest user request:\n{}",
        role,
        command.data["title"].as_str().unwrap_or("User request"),
        command.data["instruction"]
            .as_str()
            .or_else(|| command.data["content"].as_str())
            .unwrap_or_default()
    );
    let response_instructions = match super::response_instructions::current(
        definition
            .as_ref()
            .and_then(|r| r["response_instructions"].as_str()),
    ) {
        Ok(text) => text,
        Err(error) => {
            emit(state, credential, command, "agent.error", &error).await;
            return;
        }
    };
    let prompt = format!(
        "Registered Agent response requirements:\n{response_instructions}\n{history}\n{prompt}"
    );
    if let Some(id) = message_id {
        let saved = state
            .store
            .transaction(|data| {
                if let Some(mut row) = data.get("runs", &id.to_string()).cloned() {
                    if row["status"] == "interrupted" {
                        return Err("run interrupted".into());
                    }
                    row["status"] = json!("running");
                    row["prompt"] = json!(prompt);
                    data.set("runs", &id.to_string(), row);
                }
                Ok(())
            })
            .await;
        if let Err(error) = saved {
            emit(state, credential, command, "agent.error", &error).await;
            return;
        }
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<RuntimeEvent>();
    let task = async {
        let workspace = super::project_workspace::execution_settings(&command.data["workspace"])?;
        workspace
            .scope(async {
                let catalog = if let Some(value) = command.data.get("skills") {
                    agent_runtime::skills::SkillCatalog::new(
                        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?,
                    )?
                } else {
                    skills::for_agent(state, credential.project_id, &credential.client_id).await?
                };
                let definitions = catalog.definitions();
                if let Some(id) = message_id {
                    state
                        .store
                        .transaction(|data| {
                            if let Some(mut row) = data.get("runs", &id.to_string()).cloned() {
                                row["skills"] = json!(definitions);
                                data.set("runs", &id.to_string(), row);
                            }
                            Ok(())
                        })
                        .await?;
                }
                let runtime = RuntimeFactory::from_config_with_policy(
                    match configured {
                        Some(config) => config.clone(),
                        None => super::agent_configuration::AgentConfiguration::runtime(
                            provider,
                            definition.as_ref(),
                        )?,
                    }
                    .with_workspace(),
                    catalog,
                    agent_runtime::skills::ExecutionPolicy::from_env()?,
                    super::tool_policy::for_group(
                        state,
                        credential.project_id,
                        "business",
                        &credential.client_id,
                        command.data["capability_project"].as_str(),
                    )
                    .await?,
                )?;
                agent_runtime::context::Guidance::scope(
                    super::steering::inbox(
                        credential.project_id,
                        command.data["capability_project"]
                            .as_str()
                            .unwrap_or("unbound"),
                    ),
                    agent_runtime::context::DocumentAccess::scope(
                        Arc::new(crate::documents::Documents::new()),
                        agent_runtime::context::HistoryAccess::scope(
                            source,
                            runtime.run_events(&prompt, &mut move |chunk| {
                                let _ = tx.send(chunk);
                            }),
                        ),
                    ),
                )
                .await
            })
            .await
    };
    let mode = definition
        .as_ref()
        .and_then(|r| r["permission_mode"].as_str())
        .and_then(|v| agent_runtime::permissions::PermissionMode::parse(v).ok())
        .unwrap_or_default();
    let global_allowlist = state.store.get("settings", "command_allowlist").await;
    let allowlist = global_allowlist
        .as_ref()
        .and_then(|r| r.get("commands"))
        .map(|v| {
            serde_json::from_value::<Vec<String>>(v.clone())
                .map_err(|e| e.to_string())
                .and_then(agent_runtime::permissions::CommandAllowlist::new)
        })
        .transpose()
        .map(|v| v.unwrap_or_default());
    let task = mode.scope(async {
        let work = allowlist?.scope(task);
        if let Some(group) = command.data["capability_project"].as_str() {
            agent_runtime::workspace::with_conversation_approval(
                format!("{}:{}", credential.project_id, group),
                work,
            )
            .await
        } else {
            work.await
        }
    });
    let process_scope = agent_runtime::execution::SessionScope::new(
        credential.project_id.to_string(),
        command.data["capability_project"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("session:{}", command.session_id)),
        credential.client_id.clone(),
    );
    let task = process_scope.run(task);
    tokio::pin!(task);
    let mut cancellation = tokio::time::interval(Duration::from_millis(100));
    let result = loop {
        tokio::select! {
            _ = cancellation.tick(), if message_id.is_some() => {
                if conversation::stopped(state,message_id.unwrap()).await || !state.sessions.lock().await.contains_key(&command.session_id) {
                    if let Some(session)=state.sessions.lock().await.get_mut(&command.session_id) {session.requests.remove(&message_id.unwrap());}
                    return;
                }
            }
            result = &mut task => break result,
            Some(event) = rx.recv() => emit_progress(state, credential, command, &event).await,
        }
    };
    while let Ok(chunk) = rx.try_recv() {
        emit_progress(state, credential, command, &chunk).await;
    }
    match result {
        Ok(answer) => {
            emit(state, credential, command, "agent.message", &answer).await;
            emit(state, credential, command, "agent.done", "").await;
        }
        Err(error) => emit(state, credential, command, "agent.error", &error).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn local_child_executes_without_a_client_process() {
        let store = storage::Store::memory();
        let state = AppState {
            policy_store: policy_store::Store::memory(),
            control_pending: Default::default(),
            sessions: Default::default(),
            clients: Default::default(),
            store,
            node_id: "node-local".into(),
            links: Arc::new(vec![]),
            link_status: Default::default(),
        };
        let project = Uuid::new_v4();
        let session = Uuid::new_v4();
        let message = Uuid::new_v4();
        let (events, mut rx) = broadcast::channel(32);
        state.sessions.lock().await.insert(
            session,
            Session {
                project_id: project,
                client_id: Some("researcher".into()),
                events,
                requests: HashMap::from([(message, "researcher".into())]),
            },
        );
        start(
            state.clone(),
            project,
            Child {
                client_id: "researcher".into(),
                role: "research".into(),
                provider: "mock".into(),
            },
        )
        .await;
        let sender = state.clients.lock().await[&(project, "researcher".into())]
            .sender
            .clone();
        sender
            .send(event(
                session,
                "command",
                json!({"message_id":message,"content":"hello"}),
            ))
            .await
            .unwrap();
        let mut answer = String::new();
        loop {
            let output = tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .unwrap()
                .unwrap();
            if output.kind == "agent.message" {
                answer = output.data["content"].as_str().unwrap().into();
            }
            if output.kind == "agent.done" {
                break;
            }
        }
        assert!(answer.contains("Mock Agent") && answer.contains("未调用模型"));
        assert!(!answer.contains("hello") && !answer.contains("Your registered role"));
    }
}
