use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch},
    Json, Router,
};
use chrono::{DateTime, Utc};
use crabot_domain::{CapabilitySource, SessionId, TaskGraph};
use crabot_runtime::{Runtime, RuntimeService};
use crabot_storage::{event_kind_payload, event_kind_type, graph_edges, MemoryStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

pub type ApiRuntimeService = RuntimeService<MemoryStore>;

#[derive(Clone)]
pub struct ApiState {
    pub service: Arc<ApiRuntimeService>,
}

impl Default for ApiState {
    fn default() -> Self {
        let store = Arc::new(MemoryStore::new());
        Self {
            service: Arc::new(RuntimeService::new(Runtime::default(), store)),
        }
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/:session_id", get(get_session))
        .route("/api/sessions/:session_id/graph", get(get_graph))
        .route("/api/sessions/:session_id/events", get(list_events))
        .route("/api/sessions/:session_id/replay", get(get_replay))
        .route("/api/capabilities", get(list_capabilities))
        .route(
            "/api/capabilities/:source/:name/config",
            patch(update_capability_config),
        )
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn serve(addr: &str) -> anyhow::Result<()> {
    let state = ApiState::default();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn list_sessions(
    State(state): State<ApiState>,
) -> Result<Json<Vec<SessionSummaryDto>>, ApiError> {
    let sessions = state.service.list_sessions().await?;
    Ok(Json(
        sessions.into_iter().map(SessionSummaryDto::from).collect(),
    ))
}

async fn create_session(
    State(state): State<ApiState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionDetailDto>, ApiError> {
    let persisted = state.service.create_session_plan(req.objective).await?;
    let session_id = persisted.snapshot.session.id;
    let record = state
        .service
        .load_session(session_id)
        .await?
        .ok_or_else(|| ApiError::not_found("session not found after create"))?;
    let graph = persisted.snapshot.graph.map(TaskGraphDto::from_graph);
    Ok(Json(SessionDetailDto {
        session: SessionSummaryDto::from(record),
        graph,
        events: persisted
            .stored_events
            .into_iter()
            .map(RuntimeEventDto::from)
            .collect(),
    }))
}

async fn get_session(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionSummaryDto>, ApiError> {
    let record = state
        .service
        .load_session(session_id)
        .await?
        .ok_or_else(|| ApiError::not_found("session not found"))?;
    Ok(Json(SessionSummaryDto::from(record)))
}

async fn get_graph(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<TaskGraphDto>, ApiError> {
    let graph = state
        .service
        .load_graph(session_id)
        .await?
        .ok_or_else(|| ApiError::not_found("graph not found"))?;
    Ok(Json(TaskGraphDto::from_graph(graph)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventQuery {
    after_seq: Option<i64>,
    limit: Option<usize>,
}

async fn list_events(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<EventQuery>,
) -> Result<Json<Vec<RuntimeEventDto>>, ApiError> {
    let events = state
        .service
        .list_events(session_id, query.after_seq, query.limit.unwrap_or(100))
        .await?;
    Ok(Json(
        events.into_iter().map(RuntimeEventDto::from).collect(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplayQuery {
    until_seq: Option<i64>,
}

async fn get_replay(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<ReplayQuery>,
) -> Result<Json<Vec<ReplayFrameDto>>, ApiError> {
    let frames = state.service.replay(session_id, query.until_seq).await?;
    Ok(Json(frames.into_iter().map(ReplayFrameDto::from).collect()))
}

async fn list_capabilities(
    State(state): State<ApiState>,
) -> Result<Json<Vec<CapabilityDto>>, ApiError> {
    let capabilities = state.service.list_capabilities().await?;
    Ok(Json(
        capabilities.into_iter().map(CapabilityDto::from).collect(),
    ))
}

async fn update_capability_config(
    State(state): State<ApiState>,
    Path((source, name)): Path<(String, String)>,
    Json(req): Json<UpdateCapabilityConfigRequest>,
) -> Result<Json<CapabilityConfigDto>, ApiError> {
    let source = parse_source(&source)?;
    let config = state
        .service
        .update_capability_config(
            source,
            name,
            req.enabled,
            req.approval_override,
            req.config.unwrap_or_else(|| json!({})),
        )
        .await?;
    Ok(Json(CapabilityConfigDto::from(config)))
}

fn parse_source(source: &str) -> Result<CapabilitySource, ApiError> {
    match source {
        "Builtin" | "builtin" => Ok(CapabilitySource::Builtin),
        "Plugin" | "plugin" => Ok(CapabilitySource::Plugin),
        "Mcp" | "mcp" => Ok(CapabilitySource::Mcp),
        "Skill" | "skill" => Ok(CapabilitySource::Skill),
        "Agent" | "agent" => Ok(CapabilitySource::Agent),
        _ => Err(ApiError::bad_request("unknown capability source")),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub objective: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryDto {
    pub id: SessionId,
    pub objective: String,
    pub phase: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub current_graph_version: i64,
    pub node_count: usize,
    pub event_count: usize,
}

impl From<crabot_runtime::SessionRecord> for SessionSummaryDto {
    fn from(record: crabot_runtime::SessionRecord) -> Self {
        Self {
            id: record.session.id,
            objective: record.session.objective,
            phase: format!("{:?}", record.session.phase),
            created_at: record.session.created_at,
            updated_at: record.updated_at,
            current_graph_version: record.current_graph_version,
            node_count: record.node_count,
            event_count: record.event_count,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetailDto {
    pub session: SessionSummaryDto,
    pub graph: Option<TaskGraphDto>,
    pub events: Vec<RuntimeEventDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGraphDto {
    pub root_task_id: Uuid,
    pub nodes: Vec<TaskNodeDto>,
    pub edges: Vec<TaskEdgeDto>,
    pub status_counts: HashMap<String, usize>,
}

impl TaskGraphDto {
    pub fn from_graph(graph: TaskGraph) -> Self {
        Self {
            root_task_id: graph.root_task,
            edges: graph_edges(&graph)
                .into_iter()
                .map(|(source, target)| TaskEdgeDto { source, target })
                .collect(),
            status_counts: graph.status_counts().into_iter().collect(),
            nodes: graph.nodes.into_iter().map(TaskNodeDto::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskNodeDto {
    pub id: Uuid,
    pub title: String,
    pub goal: String,
    pub department: String,
    pub assignee_source: String,
    pub assignee_name: String,
    pub status: String,
}

impl From<crabot_domain::TaskNode> for TaskNodeDto {
    fn from(node: crabot_domain::TaskNode) -> Self {
        Self {
            id: node.id,
            title: node.title,
            goal: node.goal,
            department: node.department,
            assignee_source: format!("{:?}", node.assignee.source),
            assignee_name: node.assignee.name,
            status: format!("{:?}", node.status),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEdgeDto {
    pub source: Uuid,
    pub target: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEventDto {
    pub id: Uuid,
    pub seq: i64,
    pub session_id: Uuid,
    pub task_id: Option<Uuid>,
    pub at: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
}

impl From<crabot_runtime::StoredRuntimeEvent> for RuntimeEventDto {
    fn from(stored: crabot_runtime::StoredRuntimeEvent) -> Self {
        Self {
            id: stored.event.id,
            seq: stored.seq,
            session_id: stored.event.session_id,
            task_id: stored.event.task_id,
            at: stored.event.at,
            event_type: event_kind_type(&stored.event.kind).to_string(),
            payload: event_kind_payload(&stored.event.kind),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayFrameDto {
    pub seq: i64,
    pub at: DateTime<Utc>,
    pub event_type: String,
    pub payload: Value,
    pub phase: Option<String>,
    pub status_counts: HashMap<String, usize>,
}

impl From<crabot_runtime::ReplayFrame> for ReplayFrameDto {
    fn from(frame: crabot_runtime::ReplayFrame) -> Self {
        Self {
            seq: frame.seq,
            at: frame.at,
            event_type: event_kind_type(&frame.event).to_string(),
            payload: event_kind_payload(&frame.event),
            phase: frame.phase.map(|phase| format!("{:?}", phase)),
            status_counts: frame.status_counts.into_iter().collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDto {
    pub source: String,
    pub name: String,
    pub description: String,
    pub permission: String,
    pub enabled: bool,
    pub approval_override: Option<String>,
    pub config: Value,
}

impl From<crabot_runtime::CapabilityView> for CapabilityDto {
    fn from(view: crabot_runtime::CapabilityView) -> Self {
        Self {
            source: format!("{:?}", view.reference.source),
            name: view.reference.name,
            description: view.description,
            permission: view.permission,
            enabled: view.enabled,
            approval_override: view.approval_override,
            config: view.config,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCapabilityConfigRequest {
    pub enabled: bool,
    pub approval_override: Option<String>,
    pub config: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityConfigDto {
    pub source: String,
    pub name: String,
    pub enabled: bool,
    pub approval_override: Option<String>,
    pub config: Value,
    pub updated_at: DateTime<Utc>,
}

impl From<crabot_runtime::CapabilityConfig> for CapabilityConfigDto {
    fn from(config: crabot_runtime::CapabilityConfig) -> Self {
        Self {
            source: format!("{:?}", config.source),
            name: config.name,
            enabled: config.enabled,
            approval_override: config.approval_override,
            config: config.config,
            updated_at: config.updated_at,
        }
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: err.into().to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": self.message,
            })),
        )
            .into_response()
    }
}
