use crate::{AppState, a2a::control, management::Manager};
use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query},
    routing::post,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
pub(super) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/documents", post(action))
        .route("/v1/documents/upload", post(upload))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .layer(Extension(manager))
}
async fn action(
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.documents(input)
        .await
        .map(Json)
        .map_err(control::api_error)
}
#[derive(Deserialize)]
struct Upload {
    name: String,
}
async fn upload(
    Extension(m): Extension<Arc<Manager>>,
    Query(input): Query<Upload>,
    body: Bytes,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let name = input
        .name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("document")
        .to_string();
    let result = async { m.document_service().upload(name, body.to_vec()).await };
    result.await.map(Json).map_err(control::api_error)
}
