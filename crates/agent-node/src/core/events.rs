use crate::*;
pub(crate) async fn persist_event(
    state: &AppState,
    project_id: Uuid,
    channel: &str,
    event: &WireEvent,
) -> bool {
    let result = async {
        let run = match event.data["message_id"].as_str() {
            Some(id) => state.store.run_meta(id).await,
            None => None,
        };
        if event.kind.starts_with("agent.")
            && run.as_ref().is_some_and(|r| r.status.as_deref() == Some("interrupted"))
        {
            return Err("late output from interrupted run".into());
        }
        // Only consult the session row when the message id did not already resolve; in a
        // group run both are the same key, and this runs once per streamed token.
        let group = match run.as_ref().and_then(|r| r.group_id.clone()) {
            Some(group) => Some(group),
            None => state
                .store
                .run_meta(&event.session_id.to_string())
                .await
                .and_then(|r| r.group_id),
        };
        let chat = group
            .map(|g| format!("group:{g}"))
            .unwrap_or_else(|| channel.to_string());
        let rows = vec![json!({"id":event.id,"project_id":project_id,"channel":channel,"type":event.kind,"payload":event,"created_at":storage::now().to_string()})];
        // The knowledge index is the only chat-record store: every meaningful event
        // lands here, assigned a `seq` under the same lock as the write. Streaming
        // fragments (`agent.delta`, `agent.progress`) are dropped by the index and
        // never touch it.
        storage::knowledge::persist(project_id, &chat, &rows)?;
        if matches!(event.kind.as_str(), "agent.done" | "agent.error") {
            state
                .store
                .transaction(|data| {
                    conversation::record(data, event);
                    Ok(())
                })
                .await?;
        }
        Ok::<_, String>(())
    }
    .await;
    if let Err(error) = result {
        eprintln!("history persistence failed: {error}");
        return false;
    }
    true
}
pub(crate) fn event(session_id: Uuid, kind: impl Into<String>, data: Value) -> WireEvent {
    WireEvent {
        id: Uuid::new_v4(),
        kind: kind.into(),
        session_id,
        data,
    }
}
