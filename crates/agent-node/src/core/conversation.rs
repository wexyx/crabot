//! Durable, provider-independent context. Process lifetime is not conversation lifetime.
use crate::*;

pub fn record(data: &mut storage::Data, event: &WireEvent) {
    let Some(key) = event.data["message_id"].as_str() else {
        return;
    };
    if let Some(mut task) = data.get("runs", key).cloned() {
        let status = match event.kind.as_str() {
            "agent.done" => "completed",
            "agent.error" => "failed",
            _ => return,
        };
        if task["status"] != "interrupted" && !(status == "completed" && task["status"] == "failed")
        {
            task["status"] = json!(status);
            data.set("runs", key, task);
        }
    }
}

pub async fn recover(state: &AppState) -> Result<(), String> {
    for mut row in state.store.list("management_sessions").await {
        if row["status"] == "running" {
            let project =
                Uuid::parse_str(&storage::field(&row, "project_id")).map_err(|e| e.to_string())?;
            let history =
                crate::core::indexed_history::IndexedHistory::new(project, "admin".into(), None);
            let (last, _) = history.rows(None, 0, u64::MAX, 1).await?;
            match last.last().and_then(|e| e["type"].as_str()) {
                Some("completed") => row["status"] = json!("completed"),
                Some("failed") => row["status"] = json!("failed"),
                _ => {
                    crate::storage::knowledge::persist(
                        project,
                        "admin",
                        &[
                            json!({"type":"failed","message":"Process restarted; inspect partial tool effects before retry."}),
                        ],
                    )?;
                    row["status"] = json!("interrupted");
                }
            }
            let id = storage::field(&row, "id");
            state
                .store
                .transaction(|d| {
                    d.set("management_sessions", &id, row);
                    Ok(())
                })
                .await?;
        }
    }
    state
        .store
        .transaction(|data| {
            for mut row in data.list("management_approvals") {
                if row["status"] == "executing" {
                    row["status"] = json!("interrupted");
                    let key = storage::field(&row, "id");
                    data.set("management_approvals", &key, row);
                }
            }
            for mut row in data.list("runs") {
                if matches!(row["status"].as_str(), Some("queued" | "running")) {
                    row["status"] = json!("interrupted");
                    row["reason"] =
                        json!("process restarted; inspect prior tool effects before continuing");
                    let key = storage::field(&row, "id");
                    data.set("runs", &key, row);
                }
            }
            Ok(())
        })
        .await
}

// Preserve durable records; provider context policy budgets the actual model request.
pub async fn context(
    state: &AppState,
    project: Uuid,
    session: Uuid,
    before: Option<Uuid>,
    agent: &str,
) -> Result<String, String> {
    let run = state.store.get("runs", &session.to_string()).await;
    let chat = run
        .as_ref()
        .and_then(|r| r["group_id"].as_str())
        .map(|g| format!("group:{g}"))
        .unwrap_or_else(|| format!("session:{session}"));
    // Read from the knowledge index for sequential history with summary awareness.
    // Records covered by this agent's summaries are excluded; its latest summary is
    // included instead, ahead of everything written after it.
    let history = crate::core::indexed_history::IndexedHistory::new(
        project,
        chat.clone(),
        Some(agent.to_string()),
    );
    history
        .context_prompt(before.map(|id| id.to_string()))
        .await
}

pub async fn stopped(state: &AppState, id: Uuid) -> bool {
    // Polled on a timer and per streamed token: never clone the whole run row here.
    state
        .store
        .run_meta(&id.to_string())
        .await
        .is_some_and(|r| r.status.as_deref() == Some("interrupted"))
}

pub(crate) async fn interrupt_session(state: &AppState, id: Uuid) -> Result<Value, String> {
    let (project, events) = state
        .sessions
        .lock()
        .await
        .get(&id)
        .map(|s| (s.project_id, s.events.clone()))
        .ok_or("unknown session".to_string())?;
    let rows: Vec<_> = state
        .store
        .list("runs")
        .await
        .into_iter()
        .filter(|r| {
            r["session_id"] == json!(id)
                && matches!(r["status"].as_str(), Some("queued" | "running"))
        })
        .collect();
    // Old remote clients have no cancellation acknowledgement. Never claim their tools stopped.
    if rows.iter().any(|r| r["local"] != true) {
        return Err("remote cancellation unsupported".to_string());
    }
    for row in &rows {
        if let Some(group) = row["group_id"].as_str() {
            agent_runtime::execution::ProcessSessions::global()
                .stop_chat(&project.to_string(), group);
        }
    }
    state
        .store
        .transaction(|data| {
            for row in &rows {
                let key = storage::field(row, "id");
                if let Some(mut current) = data.get("runs", &key).cloned() {
                    if matches!(current["status"].as_str(), Some("queued" | "running")) {
                        current["status"] = json!("interrupted");
                        data.set("runs", &key, current);
                    }
                }
            }
            Ok(())
        })
        .await
        .map_err(|_| "persistence failed".to_string())?;
    if let Some(session) = state.sessions.lock().await.get_mut(&id) {
        session
            .requests
            .retain(|message_id, _| !rows.iter().any(|r| r["id"] == json!(message_id)));
    }
    let output = event(
        id,
        "task.interrupted",
        json!({"content":"Interrupted; partial progress retained. Send a new message to continue."}),
    );
    persist_event(&state, project, &format!("session:{id}"), &output).await;
    let _ = events.send(output);
    Ok(json!({"status":"interrupted","runs":rows.len()}))
}
