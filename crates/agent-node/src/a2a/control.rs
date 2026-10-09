//! Parent-initiated RPC travels down existing mounted SSE channels. Children can only reply.
use crate::*;
use tokio::sync::oneshot;

pub struct Pending {
    project: Uuid,
    client: String,
    reply: oneshot::Sender<Result<Value, String>>,
}
struct PendingLease(AppState, Uuid);
impl Drop for PendingLease {
    fn drop(&mut self) {
        let state = self.0.clone();
        let id = self.1;
        tokio::spawn(async move {
            state.control_pending.lock().await.remove(&id);
        });
    }
}
#[cfg(test)]
impl Pending {
    pub fn fixture(
        project: Uuid,
        client: &str,
        reply: oneshot::Sender<Result<Value, String>>,
    ) -> Self {
        Self {
            project,
            client: client.into(),
            reply,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: Uuid,
    pub path: Vec<String>,
    pub op: String,
    pub input: Value,
    pub visited: Vec<String>,
}
#[derive(Deserialize)]
pub struct Reply {
    pub id: Uuid,
    pub result: Option<Value>,
    pub error: Option<String>,
}
pub fn validate_path(path: &[String]) -> Result<(), String> {
    if path.len() > 8
        || path.iter().any(|s| {
            s.is_empty()
                || s.len() > 255
                || s == ".."
                || s == "."
                || s.contains('/')
                || s.contains('\\')
        })
    {
        return Err("invalid descendant path".into());
    }
    Ok(())
}
pub fn api_error(error: String) -> (StatusCode, Json<Value>) {
    let status = if error.contains("version_conflict") {
        StatusCode::CONFLICT
    } else if error == "not_found" {
        StatusCode::NOT_FOUND
    } else if error.contains("forbidden") {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(json!({"error":error})))
}
pub async fn reply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Reply>,
) -> Result<StatusCode, StatusCode> {
    let actor = auth_client(&state, &headers).await?;
    finish(&state, &actor, input).await
}

pub(crate) async fn finish(
    state: &AppState,
    actor: &Credential,
    input: Reply,
) -> Result<StatusCode, StatusCode> {
    let mut pending = state.control_pending.lock().await;
    let entry = pending.get(&input.id).ok_or(StatusCode::NOT_FOUND)?;
    if entry.project != actor.project_id || entry.client != actor.client_id {
        return Err(StatusCode::FORBIDDEN);
    }
    let entry = pending.remove(&input.id).unwrap();
    let _ = entry.reply.send(match input.error {
        Some(e) => Err(e),
        None => input.result.ok_or("missing RPC result".into()),
    });
    Ok(StatusCode::ACCEPTED)
}
pub fn call<'a>(
    state: &'a AppState,
    p: Uuid,
    path: Vec<String>,
    op: String,
    input: Value,
    mut visited: Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send + 'a>> {
    Box::pin(async move {
        validate_path(&path)?;
        if visited.len() >= 8 || visited.contains(&state.node_id) {
            return Err("control routing loop or hop limit".into());
        }
        visited.push(state.node_id.clone());
        if path.is_empty() {
            return crate::policies::operate(state, p, &op, input, &visited).await;
        }
        let child = &path[0];
        let sender = state
            .clients
            .lock()
            .await
            .get(&(p, child.clone()))
            .filter(|c| {
                c.node_id.is_some()
                    && c.node_id.as_deref() != Some(&state.node_id)
                    && !c.sender.is_closed()
            })
            .map(|c| c.sender.clone())
            .ok_or("descendant offline or not a Crabot")?;
        let id = Uuid::new_v4();
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = state.control_pending.lock().await;
            if pending.len() >= 128 {
                return Err("too many pending control requests".into());
            }
            pending.insert(
                id,
                Pending {
                    project: p,
                    client: child.clone(),
                    reply: tx,
                },
            );
        }
        let _lease = PendingLease(state.clone(), id);
        let request = Request {
            id,
            path: path[1..].to_vec(),
            op: op.clone(),
            input,
            visited,
        };
        if sender
            .try_send(event(
                Uuid::nil(),
                "control.request",
                json!({"request":request}),
            ))
            .is_err()
        {
            state.control_pending.lock().await.remove(&id);
            return Err("descendant queue full".into());
        }
        let result = if matches!(op.as_str(), "task.run" | "agent.invoke") {
            Ok(rx.await)
        } else {
            tokio::time::timeout(Duration::from_secs(60), rx).await
        };
        state.control_pending.lock().await.remove(&id);
        result
            .map_err(|_| "descendant RPC timed out")?
            .map_err(|_| "descendant RPC closed")?
    })
}

pub async fn receive(
    state: AppState,
    project: Uuid,
    parent: String,
    request: Request,
) -> Result<Value, String> {
    if request.visited.last() != Some(&parent) {
        return Err("forbidden: parent identity mismatch".into());
    }
    call(
        &state,
        project,
        request.path,
        request.op,
        request.input,
        request.visited,
    )
    .await
}
