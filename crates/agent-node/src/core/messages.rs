use super::error::{Detail, ErrorKind};
use crate::{AppState, SendMessage, conversation, event, persist_event, skills};
use serde_json::{Value, json};
use uuid::Uuid;
pub(crate) async fn send(
    state: &AppState,
    id: Uuid,
    input: SendMessage,
) -> Result<Value, (ErrorKind, Detail)> {
    // Group orchestration supplies durable history, not just user input. Let the
    // provider budget that context; keep the public message size limit unchanged.
    let limit = if super::project_execution::ProjectExecution::current().is_some() {
        16 * 1024 * 1024
    } else {
        192 * 1024
    };
    if input.content.trim().is_empty() || input.content.len() > limit {
        return Err((
            ErrorKind::Invalid,
            Detail(json!({"error":format!("content required, maximum {limit} bytes")})),
        ));
    }
    let message_id = Uuid::new_v4();
    let mut route = input.route.unwrap_or_default();
    route
        .enter(&state.node_id)
        .map_err(|error| (ErrorKind::Conflict, Detail(json!({"error":error}))))?;
    let (target, events, project_id) = {
        let mut sessions = state.sessions.lock().await;
        let session = sessions.get_mut(&id).ok_or((
            ErrorKind::NotFound,
            Detail(json!({"error":"unknown session"})),
        ))?;
        if input.client_id.is_some() {
            session.client_id = input.client_id.clone();
        }
        (
            session.client_id.clone(),
            session.events.clone(),
            session.project_id,
        )
    };
    let client_id = match target {
        Some(id) => id,
        None => state
            .clients
            .lock()
            .await
            .iter()
            .filter(|((project, _), client)| *project == project_id && !client.sender.is_closed())
            .map(|((_, id), _)| id.clone())
            .min()
            .ok_or((
                ErrorKind::Conflict,
                Detail(json!({"error":"no connected client"})),
            ))?,
    };
    let test = super::agent_tests::is_test(state, project_id).await;
    let definition = state
        .store
        .get("local_agents", &format!("{project_id}:{client_id}"))
        .await;
    let local_test = test && definition.is_some();
    if !local_test
        && state
            .store
            .get("local_agents", &format!("{project_id}:{client_id}"))
            .await
            .is_some_and(|r| r["enabled"] == false)
    {
        return Err((
            ErrorKind::Conflict,
            Detail(json!({"error":"Agent 已停用，不接收新任务","client_id":client_id})),
        ));
    }
    let content = input.content;
    let user_event = event(
        id,
        "message.created",
        json!({"message_id":message_id,"role":"user","content":content.clone(),"route":route}),
    );
    let mut command = event(
        id,
        "command",
        json!({"message_id":message_id,"content":content,"route":route}),
    );
    let group = super::project_execution::ProjectExecution::current();
    command.data["capability_project"] = json!(group);
    command.data["history_before"] = json!(super::project_execution::ProjectExecution::run());
    command.data["workspace"] = super::project_execution::ProjectExecution::workspace();
    let catalog = skills::for_group(&state, project_id, &client_id, group.as_deref())
        .await
        .map_err(|error| (ErrorKind::Internal, Detail(json!({"error":error}))))?;
    command.data["skills"] = json!(catalog.definitions());
    let sender = if local_test {
        None
    } else {
        Some(
            state
                .clients
                .lock()
                .await
                .get(&(project_id, client_id.clone()))
                .filter(|client| !client.sender.is_closed())
                .map(|client| client.sender.clone())
                .ok_or((
                    ErrorKind::Conflict,
                    Detail(json!({"error":"client disconnected","client_id":client_id})),
                ))?,
        )
    };
    let is_local = state
        .store
        .list("local_agents")
        .await
        .iter()
        .any(|r| r["project_id"] == json!(project_id) && r["client_id"] == client_id);
    let history = conversation::context(&state, project_id, id, None, &client_id)
        .await
        .map_err(|error| (ErrorKind::Conflict, Detail(json!({"error":error}))))?;
    if !is_local {
        if !state
            .sessions
            .lock()
            .await
            .get(&id)
            .is_some_and(|s| s.requests.is_empty())
        {
            return Err((
                ErrorKind::Conflict,
                Detail(json!({"error":"remote session busy; wait before continuing"})),
            ));
        }
        command.data["content"] = json!(format!("{history}\nLatest user request:\n{content}"));
    }
    state.store.insert("runs", &message_id.to_string(), json!({"id":message_id,"session_id":id,"project_id":project_id,"client_id":client_id,"status":"queued","local":is_local,"content":content,"skills":command.data["skills"]})).await
        .map_err(|error|(ErrorKind::Internal,Detail(json!({"error":error}))))?;
    if let Some(session) = state.sessions.lock().await.get_mut(&id) {
        session.requests.insert(message_id, client_id.clone());
    }
    if !persist_event(&state, project_id, &format!("session:{id}"), &user_event).await {
        if let Some(session) = state.sessions.lock().await.get_mut(&id) {
            session.requests.remove(&message_id);
        }
        return Err((
            ErrorKind::Internal,
            Detail(json!({"error":"could not persist user request; execution not started"})),
        ));
    }
    let _ = events.send(user_event.clone());
    let dispatch = if let Some(definition) = definition.filter(|_| local_test) {
        let state = state.clone();
        let credential = crate::Credential {
            project_id,
            client_id: client_id.clone(),
            role: definition["role"].as_str().unwrap_or_default().into(),
        };
        let provider = definition["provider"].as_str().unwrap_or("mock").to_owned();
        tokio::spawn(async move {
            super::local::execute(&state, &credential, &provider, &command, None).await;
        });
        Ok(())
    } else {
        sender.expect("normal dispatch sender").try_send(command)
    };
    if let Err(error) = dispatch {
        let failure = event(
            id,
            "agent.error",
            json!({"message_id":message_id,"content":"dispatch failed; send again to retry"}),
        );
        persist_event(&state, project_id, &format!("session:{id}"), &failure).await;
        if let Some(session) = state.sessions.lock().await.get_mut(&id) {
            session.requests.remove(&message_id);
        }
        return Err((
            ErrorKind::Conflict,
            Detail(json!({"error":format!("agent unavailable or queue full: {error}")})),
        ));
    }
    // Only durable sessions have a file record; temporary group routes stay in memory.
    if let Err(error) = state
        .store
        .transaction(|data| {
            if let Some(mut row) = data.get("sessions", &id.to_string()).cloned() {
                row["client_id"] = json!(client_id);
                data.set("sessions", &id.to_string(), row);
            }
            Ok(())
        })
        .await
    {
        eprintln!("session persistence failed: {error}");
    }
    Ok(json!({"session_id":id,"message_id":message_id,"client_id":client_id}))
}
