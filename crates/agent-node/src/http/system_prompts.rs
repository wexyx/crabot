use crate::{AppState, control, management::Manager};
use axum::{Extension, Json, Router, routing::get};
use serde_json::Value;
use std::sync::Arc;
type Reply = Result<Json<Value>, (axum::http::StatusCode, Json<Value>)>;

pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/config/prompts", get(read).put(save))
        .layer(Extension(manager))
}
async fn read(Extension(m): Extension<Arc<Manager>>) -> Reply {
    m.core()
        .system_prompts()
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn save(Extension(m): Extension<Arc<Manager>>, Json(input): Json<Value>) -> Reply {
    m.core()
        .save_system_prompt(input)
        .await
        .map(Json)
        .map_err(control::api_error)
}
