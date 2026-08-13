use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crabot_domain::{
    ArtifactRef, CapabilityRef, CapabilitySource, RuntimeEvent, RuntimeEventKind, Session,
    SessionId, SessionPhase, TaskGraph, TaskId, TaskStatus,
};
use crabot_runtime::{
    CapabilityConfig, CapabilityConfigStore, EventStore, SessionRecord, SessionStore,
    StoredRuntimeEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub const SQLITE_MIGRATIONS: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  objective TEXT NOT NULL,
  phase TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  current_graph_version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS task_graph_versions (
  session_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  root_task_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (session_id, version)
);

CREATE TABLE IF NOT EXISTS task_nodes (
  session_id TEXT NOT NULL,
  graph_version INTEGER NOT NULL,
  task_id TEXT NOT NULL,
  title TEXT NOT NULL,
  goal TEXT NOT NULL,
  department TEXT NOT NULL,
  assignee_source TEXT NOT NULL,
  assignee_name TEXT NOT NULL,
  status TEXT NOT NULL,
  artifacts_json TEXT NOT NULL,
  PRIMARY KEY (session_id, graph_version, task_id)
);

CREATE TABLE IF NOT EXISTS task_dependencies (
  session_id TEXT NOT NULL,
  graph_version INTEGER NOT NULL,
  task_id TEXT NOT NULL,
  depends_on_task_id TEXT NOT NULL,
  PRIMARY KEY (session_id, graph_version, task_id, depends_on_task_id)
);

CREATE TABLE IF NOT EXISTS runtime_events (
  session_id TEXT NOT NULL,
  seq INTEGER NOT NULL,
  event_id TEXT NOT NULL,
  task_id TEXT,
  kind TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  at TEXT NOT NULL,
  PRIMARY KEY (session_id, seq)
);

CREATE TABLE IF NOT EXISTS capability_configs (
  source TEXT NOT NULL,
  name TEXT NOT NULL,
  enabled INTEGER NOT NULL,
  approval_override TEXT,
  config_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (source, name)
);
"#;

#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    inner: Arc<RwLock<MemoryStoreState>>,
}

#[derive(Debug, Clone, Default)]
struct MemoryStoreState {
    sessions: BTreeMap<SessionId, PersistedSession>,
    graphs: BTreeMap<SessionId, VersionedGraph>,
    events: BTreeMap<SessionId, Vec<StoredRuntimeEvent>>,
    capability_configs: BTreeMap<(String, String), CapabilityConfig>,
}

#[derive(Debug, Clone)]
struct PersistedSession {
    session: Session,
    updated_at: DateTime<Utc>,
    current_graph_version: i64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct VersionedGraph {
    version: i64,
    graph: TaskGraph,
    created_at: DateTime<Utc>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn migrations_sql(&self) -> &'static str {
        SQLITE_MIGRATIONS
    }

    pub fn save_snapshot(
        &self,
        session: &Session,
        graph: &TaskGraph,
        events: &[RuntimeEvent],
    ) -> Result<Vec<StoredRuntimeEvent>> {
        self.save_session(session, graph)
            .and_then(|_| self.append_events(session.id, events))
    }

    fn read_state(&self) -> Result<std::sync::RwLockReadGuard<'_, MemoryStoreState>> {
        self.inner
            .read()
            .map_err(|_| anyhow!("memory store read lock poisoned"))
    }

    fn write_state(&self) -> Result<std::sync::RwLockWriteGuard<'_, MemoryStoreState>> {
        self.inner
            .write()
            .map_err(|_| anyhow!("memory store write lock poisoned"))
    }
}

#[async_trait]
impl SessionStore for MemoryStore {
    async fn save_session(&self, session: &Session, graph: Option<&TaskGraph>) -> Result<()> {
        self.save_session(
            session,
            graph.ok_or_else(|| anyhow!("graph is required for this storage implementation"))?,
        )
    }

    async fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        self.list_sessions()
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Option<SessionRecord>> {
        self.get_session(session_id)
    }

    async fn get_graph(&self, session_id: SessionId) -> Result<Option<TaskGraph>> {
        self.get_graph(session_id)
    }
}

#[async_trait]
impl EventStore for MemoryStore {
    async fn append_events(
        &self,
        session_id: SessionId,
        events: &[RuntimeEvent],
    ) -> Result<Vec<StoredRuntimeEvent>> {
        self.append_events(session_id, events)
    }

    async fn list_events(
        &self,
        session_id: SessionId,
        after_seq: Option<i64>,
        limit: usize,
    ) -> Result<Vec<StoredRuntimeEvent>> {
        self.list_events(session_id, after_seq, limit)
    }
}

#[async_trait]
impl CapabilityConfigStore for MemoryStore {
    async fn list_capability_configs(&self) -> Result<Vec<CapabilityConfig>> {
        self.list_capability_configs()
    }

    async fn upsert_capability_config(&self, config: CapabilityConfig) -> Result<CapabilityConfig> {
        self.upsert_capability_config(config)
    }
}

impl MemoryStore {
    pub fn save_session(&self, session: &Session, graph: &TaskGraph) -> Result<()> {
        let mut state = self.write_state()?;
        let version = state
            .graphs
            .get(&session.id)
            .map(|graph| graph.version + 1)
            .unwrap_or(1);
        state.sessions.insert(
            session.id,
            PersistedSession {
                session: session.clone(),
                updated_at: Utc::now(),
                current_graph_version: version,
            },
        );
        state.graphs.insert(
            session.id,
            VersionedGraph {
                version,
                graph: graph.clone(),
                created_at: Utc::now(),
            },
        );
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        let state = self.read_state()?;
        let mut records = Vec::new();
        for persisted in state.sessions.values() {
            let graph = state.graphs.get(&persisted.session.id);
            let events = state
                .events
                .get(&persisted.session.id)
                .map(Vec::len)
                .unwrap_or_default();
            records.push(SessionRecord {
                session: persisted.session.clone(),
                updated_at: persisted.updated_at,
                current_graph_version: persisted.current_graph_version,
                node_count: graph
                    .map(|graph| graph.graph.nodes.len())
                    .unwrap_or_default(),
                event_count: events,
            });
        }
        records.sort_by(|a, b| b.session.created_at.cmp(&a.session.created_at));
        Ok(records)
    }

    pub fn get_session(&self, session_id: SessionId) -> Result<Option<SessionRecord>> {
        let state = self.read_state()?;
        let Some(persisted) = state.sessions.get(&session_id) else {
            return Ok(None);
        };
        let graph = state.graphs.get(&session_id);
        let events = state
            .events
            .get(&session_id)
            .map(Vec::len)
            .unwrap_or_default();
        Ok(Some(SessionRecord {
            session: persisted.session.clone(),
            updated_at: persisted.updated_at,
            current_graph_version: persisted.current_graph_version,
            node_count: graph
                .map(|graph| graph.graph.nodes.len())
                .unwrap_or_default(),
            event_count: events,
        }))
    }

    pub fn get_graph(&self, session_id: SessionId) -> Result<Option<TaskGraph>> {
        let state = self.read_state()?;
        Ok(state
            .graphs
            .get(&session_id)
            .map(|graph| graph.graph.clone()))
    }

    pub fn append_events(
        &self,
        session_id: SessionId,
        events: &[RuntimeEvent],
    ) -> Result<Vec<StoredRuntimeEvent>> {
        let mut state = self.write_state()?;
        let stored = state.events.entry(session_id).or_default();
        let mut appended = Vec::new();
        for event in events {
            let seq = stored.len() as i64 + 1;
            let stored_event = StoredRuntimeEvent::from_event(seq, event.clone());
            stored.push(stored_event.clone());
            appended.push(stored_event);
        }
        Ok(appended)
    }

    pub fn list_events(
        &self,
        session_id: SessionId,
        after_seq: Option<i64>,
        limit: usize,
    ) -> Result<Vec<StoredRuntimeEvent>> {
        let state = self.read_state()?;
        let events = state.events.get(&session_id).cloned().unwrap_or_default();
        let after_seq = after_seq.unwrap_or(0);
        Ok(events
            .into_iter()
            .filter(|event| event.seq > after_seq)
            .take(limit.max(1))
            .collect())
    }

    pub fn list_capability_configs(&self) -> Result<Vec<CapabilityConfig>> {
        let state = self.read_state()?;
        Ok(state.capability_configs.values().cloned().collect())
    }

    pub fn upsert_capability_config(&self, config: CapabilityConfig) -> Result<CapabilityConfig> {
        let mut state = self.write_state()?;
        state.capability_configs.insert(
            (format!("{:?}", config.source), config.name.clone()),
            config.clone(),
        );
        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecordRow {
    pub id: String,
    pub objective: String,
    pub phase: String,
    pub created_at: String,
    pub updated_at: String,
    pub current_graph_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNodeRecordRow {
    pub session_id: String,
    pub graph_version: i64,
    pub task_id: String,
    pub title: String,
    pub goal: String,
    pub department: String,
    pub assignee_source: String,
    pub assignee_name: String,
    pub status: String,
    pub artifacts_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEventRecordRow {
    pub session_id: String,
    pub seq: i64,
    pub event_id: String,
    pub task_id: Option<String>,
    pub kind: String,
    pub payload_json: String,
    pub at: String,
}

pub fn event_kind_type(kind: &RuntimeEventKind) -> &'static str {
    match kind {
        RuntimeEventKind::SessionCreated { .. } => "session.created",
        RuntimeEventKind::PhaseChanged { .. } => "phase.changed",
        RuntimeEventKind::PlanCreated { .. } => "plan.created",
        RuntimeEventKind::TaskStatusChanged { .. } => "task.statusChanged",
        RuntimeEventKind::ToolCallStarted { .. } => "tool.started",
        RuntimeEventKind::ToolCallFinished { .. } => "tool.finished",
        RuntimeEventKind::ApprovalRequested { .. } => "approval.requested",
        RuntimeEventKind::ApprovalResolved { .. } => "approval.resolved",
        RuntimeEventKind::Message { .. } => "message",
        RuntimeEventKind::Error { .. } => "error",
    }
}

pub fn event_kind_payload(kind: &RuntimeEventKind) -> Value {
    serde_json::to_value(kind).unwrap_or(Value::Null)
}

pub fn runtime_event_from_parts(
    session_id: SessionId,
    task_id: Option<TaskId>,
    at: DateTime<Utc>,
    kind: RuntimeEventKind,
) -> RuntimeEvent {
    RuntimeEvent {
        id: Uuid::new_v4(),
        session_id,
        task_id,
        at,
        kind,
    }
}

pub fn graph_edges(graph: &TaskGraph) -> Vec<(TaskId, TaskId)> {
    graph
        .nodes
        .iter()
        .flat_map(|node| node.dependencies.iter().map(move |dep| (*dep, node.id)))
        .collect()
}

pub fn status_counts(graph: &TaskGraph) -> HashMap<String, usize> {
    graph.status_counts().into_iter().collect()
}

pub fn encode_artifacts(artifacts: &[ArtifactRef]) -> Result<String> {
    serde_json::to_string(artifacts).map_err(Into::into)
}

pub fn decode_artifacts(value: &str) -> Result<Vec<ArtifactRef>> {
    serde_json::from_str(value).map_err(Into::into)
}

pub fn parse_session_phase(value: &str) -> Result<SessionPhase> {
    serde_json::from_value(Value::String(value.to_string())).map_err(Into::into)
}

pub fn parse_task_status(value: &str) -> Result<TaskStatus> {
    serde_json::from_value(Value::String(value.to_string())).map_err(Into::into)
}

pub fn parse_capability_source(value: &str) -> Result<CapabilitySource> {
    serde_json::from_value(Value::String(value.to_string())).map_err(Into::into)
}

pub fn capability_ref(source: CapabilitySource, name: impl Into<String>) -> CapabilityRef {
    CapabilityRef::new(source, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crabot_domain::{CapabilitySource, RuntimeEventKind, TaskNode};

    #[test]
    fn saves_and_loads_session_graph_and_events() {
        let store = MemoryStore::new();
        let session = Session::new("build persistence");
        let node = TaskNode::new(
            "plan",
            "persist plan",
            "engineering",
            CapabilityRef::new(CapabilitySource::Builtin, "planner"),
        );
        let graph = TaskGraph::new(node.id, vec![node]);
        let event = RuntimeEvent::new(
            session.id,
            None,
            RuntimeEventKind::SessionCreated {
                objective: session.objective.clone(),
            },
        );

        store.save_snapshot(&session, &graph, &[event]).unwrap();

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].node_count, 1);
        assert_eq!(sessions[0].event_count, 1);
        assert!(store.get_graph(session.id).unwrap().is_some());
        assert_eq!(store.list_events(session.id, None, 10).unwrap()[0].seq, 1);
    }

    #[test]
    fn upserts_capability_config() {
        let store = MemoryStore::new();
        let config = CapabilityConfig {
            source: CapabilitySource::Builtin,
            name: "planner".to_string(),
            enabled: false,
            approval_override: Some("always".to_string()),
            config: serde_json::json!({ "mode": "safe" }),
            updated_at: Utc::now(),
        };

        store.upsert_capability_config(config.clone()).unwrap();
        assert_eq!(store.list_capability_configs().unwrap()[0].enabled, false);
    }
}
