use crate::{AppState, a2a::control, management::Manager};
use axum::{
    Extension, Json, Router,
    extract::Path,
    routing::{get, post},
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/repl/{project}/chats/{chat}/processes", get(list))
        .route(
            "/v1/repl/{project}/chats/{chat}/processes/{op}",
            post(action),
        )
        .layer(Extension(manager))
}
async fn list(
    Path((p, chat)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.process_sessions(p, &chat, "list", serde_json::json!({}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn action(
    Path((p, chat, op)): Path<(Uuid, String, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.process_sessions(p, &chat, &op, input)
        .await
        .map(Json)
        .map_err(control::api_error)
}
