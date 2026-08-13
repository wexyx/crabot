use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crabot_domain::{
    CapabilityRef, CapabilitySource, CompanyProfile, RuntimeEvent, RuntimeEventKind, Session,
    SessionId, SessionPhase, TaskGraph, TaskNode,
};
use crabot_mcp::McpServerDefinition;
use crabot_plugins::{PermissionLevel, PluginManifest};
use crabot_skills::{default_skill_catalog, SkillCatalog, SlashCommandKind};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StoredRuntimeEvent {
    pub seq: i64,
    pub event: RuntimeEvent,
}

impl StoredRuntimeEvent {
    pub fn from_event(seq: i64, event: RuntimeEvent) -> Self {
        Self { seq, event }
    }
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub session: Session,
    pub updated_at: DateTime<Utc>,
    pub current_graph_version: i64,
    pub node_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Clone)]
pub struct CapabilityConfig {
    pub source: CapabilitySource,
    pub name: String,
    pub enabled: bool,
    pub approval_override: Option<String>,
    pub config: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save_session(&self, session: &Session, graph: Option<&TaskGraph>) -> Result<()>;
    async fn list_sessions(&self) -> Result<Vec<SessionRecord>>;
    async fn get_session(&self, session_id: SessionId) -> Result<Option<SessionRecord>>;
    async fn get_graph(&self, session_id: SessionId) -> Result<Option<TaskGraph>>;
}

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append_events(
        &self,
        session_id: SessionId,
        events: &[RuntimeEvent],
    ) -> Result<Vec<StoredRuntimeEvent>>;
    async fn list_events(
        &self,
        session_id: SessionId,
        after_seq: Option<i64>,
        limit: usize,
    ) -> Result<Vec<StoredRuntimeEvent>>;
}

#[async_trait]
pub trait CapabilityConfigStore: Send + Sync {
    async fn list_capability_configs(&self) -> Result<Vec<CapabilityConfig>>;
    async fn upsert_capability_config(&self, config: CapabilityConfig) -> Result<CapabilityConfig>;
}

pub trait RuntimeStore: SessionStore + EventStore + CapabilityConfigStore {}
impl<T> RuntimeStore for T where T: SessionStore + EventStore + CapabilityConfigStore {}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub company: CompanyProfile,
    pub max_parallel_tasks: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            company: CompanyProfile::default(),
            max_parallel_tasks: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityDescriptor {
    pub reference: CapabilityRef,
    pub description: String,
    pub permission: PermissionLevel,
}

impl CapabilityDescriptor {
    pub fn builtin(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            reference: CapabilityRef::new(CapabilitySource::Builtin, name),
            description: description.into(),
            permission: PermissionLevel::ReadOnly,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    capabilities: BTreeMap<String, CapabilityDescriptor>,
}

impl CapabilityRegistry {
    pub fn register(&mut self, descriptor: CapabilityDescriptor) {
        self.capabilities
            .insert(descriptor.reference.name.clone(), descriptor);
    }

    pub fn register_plugin(&mut self, manifest: &PluginManifest) {
        for capability in &manifest.capabilities {
            self.register(CapabilityDescriptor {
                reference: CapabilityRef::new(capability.source, capability.name.clone()),
                description: capability.description.clone(),
                permission: capability.permission,
            });
        }
    }

    pub fn register_mcp_server(&mut self, server: &McpServerDefinition) {
        for tool in &server.tools {
            self.register(CapabilityDescriptor {
                reference: CapabilityRef::new(
                    CapabilitySource::Mcp,
                    format!("{}:{}", server.name, tool.name),
                ),
                description: tool.description.clone(),
                permission: PermissionLevel::ReadOnly,
            });
        }
    }

    pub fn list(&self) -> impl Iterator<Item = &CapabilityDescriptor> {
        self.capabilities.values()
    }

    pub fn get(&self, name: &str) -> Option<&CapabilityDescriptor> {
        self.capabilities.get(name)
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    pub session: Session,
    pub graph: Option<TaskGraph>,
    pub events: Vec<RuntimeEvent>,
}

#[derive(Debug, Clone)]
pub struct PersistedRuntimeSnapshot {
    pub snapshot: RuntimeSnapshot,
    pub stored_events: Vec<StoredRuntimeEvent>,
}

#[derive(Debug, Clone)]
pub struct ReplayFrame {
    pub seq: i64,
    pub at: DateTime<Utc>,
    pub event: RuntimeEventKind,
    pub phase: Option<SessionPhase>,
    pub status_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub config: RuntimeConfig,
    pub skills: SkillCatalog,
    pub capabilities: CapabilityRegistry,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Self {
        let mut capabilities = CapabilityRegistry::default();
        capabilities.register(CapabilityDescriptor::builtin(
            "planner",
            "生成可观察、可恢复的 Task DAG",
        ));
        capabilities.register(CapabilityDescriptor::builtin(
            "scorecard",
            "基于完成率、质量、效率、协作生成运行评分",
        ));

        Self {
            config,
            skills: default_skill_catalog(),
            capabilities,
        }
    }

    pub fn bootstrap_summary(&self) -> BootstrapSummary {
        BootstrapSummary {
            company: self.config.company.clone(),
            command_count: self.skills.commands().count(),
            skill_count: self.skills.skills().count(),
            capability_count: self.capabilities.list().count(),
        }
    }

    pub fn plan(&self, objective: impl Into<String>) -> RuntimeSnapshot {
        let objective = objective.into();
        let mut session = Session::new(objective.clone());
        session.phase = SessionPhase::Planning;

        let mut events = vec![
            RuntimeEvent::new(
                session.id,
                None,
                RuntimeEventKind::SessionCreated {
                    objective: objective.clone(),
                },
            ),
            RuntimeEvent::new(
                session.id,
                None,
                RuntimeEventKind::PhaseChanged {
                    phase: SessionPhase::Planning,
                },
            ),
        ];

        let graph = self.default_task_graph(&objective);
        events.push(RuntimeEvent::new(
            session.id,
            None,
            RuntimeEventKind::PlanCreated {
                node_count: graph.nodes.len(),
            },
        ));

        RuntimeSnapshot {
            session,
            graph: Some(graph),
            events,
        }
    }

    pub fn handle_slash_command(&self, input: &str) -> Result<RuntimeCommandResult> {
        let invocation = self.skills.parse(input)?;
        match invocation.kind {
            SlashCommandKind::Plan => {
                let snapshot = self.plan(invocation.args);
                Ok(RuntimeCommandResult::Snapshot(snapshot))
            }
            SlashCommandKind::Tools => Ok(RuntimeCommandResult::Text(
                self.capabilities
                    .list()
                    .map(|capability| format!("- {} ({:?}) — {}", capability.reference.name, capability.reference.source, capability.description))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
            SlashCommandKind::Skills => Ok(RuntimeCommandResult::Text(
                self.skills
                    .skills()
                    .map(|skill| format!("- /{} — {}", skill.name, skill.description))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
            SlashCommandKind::Status => Ok(RuntimeCommandResult::Text(
                "runtime: ready\nphase: waiting for objective\nplanner: available\ncapability registry: online".to_string(),
            )),
            SlashCommandKind::Run => Err(anyhow!("/run will execute a persisted plan in the next runtime phase")),
            SlashCommandKind::Approve | SlashCommandKind::Reject => Err(anyhow!("approval queue is not active yet")),
            SlashCommandKind::Replay => Err(anyhow!("session replay requires storage/event-log phase")),
            SlashCommandKind::Skill => Ok(RuntimeCommandResult::Text(format!(
                "skill /{} accepted; execution will be routed through planner runtime",
                invocation.command
            ))),
        }
    }

    fn default_task_graph(&self, objective: &str) -> TaskGraph {
        let intake = TaskNode::new(
            "分析目标与边界",
            format!("识别任务目标、风险、资源和验收标准：{}", objective),
            "research",
            CapabilityRef::new(CapabilitySource::Agent, "ceo-router"),
        );
        let planner = TaskNode::new(
            "生成执行 DAG",
            "将目标拆成可观察、可审批、可恢复的任务图",
            "engineering",
            CapabilityRef::new(CapabilitySource::Builtin, "planner"),
        )
        .depends_on(intake.id);
        let tools = TaskNode::new(
            "匹配 Skills / Tools / MCP",
            "为每个任务节点选择内置工具、插件工具或 MCP 工具，并标记审批策略",
            "ops",
            CapabilityRef::new(CapabilitySource::Skill, "toolsmith"),
        )
        .depends_on(planner.id);
        let review = TaskNode::new(
            "汇总结果与评分",
            "生成最终答复、artifacts 和 scorecard",
            "ops",
            CapabilityRef::new(CapabilitySource::Builtin, "scorecard"),
        )
        .depends_on(tools.id);

        TaskGraph::new(intake.id, vec![intake, planner, tools, review])
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(RuntimeConfig::default())
    }
}

#[derive(Clone)]
pub struct RuntimeService<S>
where
    S: RuntimeStore + 'static,
{
    runtime: Runtime,
    store: Arc<S>,
}

impl<S> RuntimeService<S>
where
    S: RuntimeStore + 'static,
{
    pub fn new(runtime: Runtime, store: Arc<S>) -> Self {
        Self { runtime, store }
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn store(&self) -> Arc<S> {
        self.store.clone()
    }

    pub async fn create_session_plan(
        &self,
        objective: impl Into<String>,
    ) -> Result<PersistedRuntimeSnapshot> {
        let snapshot = self.runtime.plan(objective);
        self.store
            .save_session(&snapshot.session, snapshot.graph.as_ref())
            .await?;
        let stored_events = self
            .store
            .append_events(snapshot.session.id, &snapshot.events)
            .await?;
        Ok(PersistedRuntimeSnapshot {
            snapshot,
            stored_events,
        })
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        self.store.list_sessions().await
    }

    pub async fn load_session(&self, session_id: SessionId) -> Result<Option<SessionRecord>> {
        self.store.get_session(session_id).await
    }

    pub async fn load_graph(&self, session_id: SessionId) -> Result<Option<TaskGraph>> {
        self.store.get_graph(session_id).await
    }

    pub async fn list_events(
        &self,
        session_id: SessionId,
        after_seq: Option<i64>,
        limit: usize,
    ) -> Result<Vec<StoredRuntimeEvent>> {
        self.store.list_events(session_id, after_seq, limit).await
    }

    pub async fn replay(
        &self,
        session_id: SessionId,
        until_seq: Option<i64>,
    ) -> Result<Vec<ReplayFrame>> {
        let limit = until_seq.unwrap_or(i64::MAX) as usize;
        let events = self.store.list_events(session_id, None, limit).await?;
        let graph = self.store.get_graph(session_id).await?;
        Ok(fold_replay(graph.as_ref(), events, until_seq))
    }

    pub async fn list_capabilities(&self) -> Result<Vec<CapabilityView>> {
        let configs = self.store.list_capability_configs().await?;
        let mut views = Vec::new();
        for descriptor in self.runtime.capabilities.list() {
            let config = configs.iter().find(|config| {
                config.source == descriptor.reference.source
                    && config.name == descriptor.reference.name
            });
            views.push(CapabilityView {
                reference: descriptor.reference.clone(),
                description: descriptor.description.clone(),
                permission: format!("{:?}", descriptor.permission),
                enabled: config.map(|config| config.enabled).unwrap_or(true),
                approval_override: config.and_then(|config| config.approval_override.clone()),
                config: config
                    .map(|config| config.config.clone())
                    .unwrap_or_else(|| serde_json::json!({})),
            });
        }
        Ok(views)
    }

    pub async fn update_capability_config(
        &self,
        source: CapabilitySource,
        name: String,
        enabled: bool,
        approval_override: Option<String>,
        config: serde_json::Value,
    ) -> Result<CapabilityConfig> {
        self.store
            .upsert_capability_config(CapabilityConfig {
                source,
                name,
                enabled,
                approval_override,
                config,
                updated_at: Utc::now(),
            })
            .await
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityView {
    pub reference: CapabilityRef,
    pub description: String,
    pub permission: String,
    pub enabled: bool,
    pub approval_override: Option<String>,
    pub config: serde_json::Value,
}

pub fn fold_replay(
    graph: Option<&TaskGraph>,
    events: Vec<StoredRuntimeEvent>,
    until_seq: Option<i64>,
) -> Vec<ReplayFrame> {
    let mut phase = None;
    let mut status_counts = graph.map(TaskGraph::status_counts).unwrap_or_default();
    let until_seq = until_seq.unwrap_or(i64::MAX);

    events
        .into_iter()
        .filter(|stored| stored.seq <= until_seq)
        .map(|stored| {
            match &stored.event.kind {
                RuntimeEventKind::PhaseChanged { phase: next_phase } => {
                    phase = Some(*next_phase);
                }
                RuntimeEventKind::TaskStatusChanged { status } => {
                    status_counts.clear();
                    status_counts.insert(format!("{:?}", status), 1);
                }
                _ => {}
            }
            ReplayFrame {
                seq: stored.seq,
                at: stored.event.at,
                event: stored.event.kind,
                phase,
                status_counts: status_counts.clone(),
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct BootstrapSummary {
    pub company: CompanyProfile,
    pub command_count: usize,
    pub skill_count: usize,
    pub capability_count: usize,
}

#[derive(Debug, Clone)]
pub enum RuntimeCommandResult {
    Text(String),
    Snapshot(RuntimeSnapshot),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_default_task_graph() {
        let runtime = Runtime::default();
        let snapshot = runtime.plan("build skill support");
        let graph = snapshot.graph.unwrap();
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(snapshot.events.len(), 3);
    }

    #[test]
    fn lists_builtin_tools_from_slash_command() {
        let runtime = Runtime::default();
        let result = runtime.handle_slash_command("/tools").unwrap();
        match result {
            RuntimeCommandResult::Text(text) => assert!(text.contains("planner")),
            RuntimeCommandResult::Snapshot(_) => panic!("expected text"),
        }
    }

    #[test]
    fn folds_phase_changes_into_replay_frames() {
        let session = Session::new("replay");
        let event = RuntimeEvent::new(
            session.id,
            None,
            RuntimeEventKind::PhaseChanged {
                phase: SessionPhase::Planning,
            },
        );
        let frames = fold_replay(None, vec![StoredRuntimeEvent::from_event(1, event)], None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].phase, Some(SessionPhase::Planning));
    }
}
