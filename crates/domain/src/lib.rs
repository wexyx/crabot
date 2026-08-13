use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub type SessionId = Uuid;
pub type TaskId = Uuid;
pub type EventId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompanyProfile {
    pub name: String,
    pub departments: Vec<DepartmentProfile>,
}

impl Default for CompanyProfile {
    fn default() -> Self {
        Self {
            name: "crabot".to_string(),
            departments: vec![
                DepartmentProfile::new("research", "Research", "搜索、调研、事实核查与资料整理"),
                DepartmentProfile::new("engineering", "Engineering", "代码生成、重构、测试与交付"),
                DepartmentProfile::new(
                    "ops",
                    "Ops / Support",
                    "工具执行、监控、日志、调度与外部集成",
                ),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepartmentProfile {
    pub key: String,
    pub name: String,
    pub mission: String,
}

impl DepartmentProfile {
    pub fn new(
        key: impl Into<String>,
        name: impl Into<String>,
        mission: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            mission: mission.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub objective: String,
    pub phase: SessionPhase,
    pub created_at: DateTime<Utc>,
}

impl Session {
    pub fn new(objective: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            objective: objective.into(),
            phase: SessionPhase::Planning,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionPhase {
    Intake,
    Planning,
    Approval,
    Executing,
    Reviewing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskNode {
    pub id: TaskId,
    pub title: String,
    pub goal: String,
    pub department: String,
    pub assignee: CapabilityRef,
    pub dependencies: Vec<TaskId>,
    pub status: TaskStatus,
    pub artifacts: Vec<ArtifactRef>,
}

impl TaskNode {
    pub fn new(
        title: impl Into<String>,
        goal: impl Into<String>,
        department: impl Into<String>,
        assignee: CapabilityRef,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            goal: goal.into(),
            department: department.into(),
            assignee,
            dependencies: Vec::new(),
            status: TaskStatus::Queued,
            artifacts: Vec::new(),
        }
    }

    pub fn depends_on(mut self, dependency: TaskId) -> Self {
        self.dependencies.push(dependency);
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Blocked,
    WaitingApproval,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskGraph {
    pub root_task: TaskId,
    pub nodes: Vec<TaskNode>,
}

impl TaskGraph {
    pub fn new(root_task: TaskId, nodes: Vec<TaskNode>) -> Self {
        Self { root_task, nodes }
    }

    pub fn status_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for node in &self.nodes {
            *counts.entry(format!("{:?}", node.status)).or_insert(0) += 1;
        }
        counts
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityRef {
    pub source: CapabilitySource,
    pub name: String,
}

impl CapabilityRef {
    pub fn new(source: CapabilitySource, name: impl Into<String>) -> Self {
        Self {
            source,
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CapabilitySource {
    Builtin,
    Plugin,
    Mcp,
    Skill,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactRef {
    pub id: Uuid,
    pub name: String,
    pub kind: ArtifactKind,
    pub uri: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArtifactKind {
    Text,
    Json,
    Log,
    File,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceScore {
    pub completion_rate: f32,
    pub quality_score: f32,
    pub efficiency: f32,
    pub collaboration_score: f32,
}

impl PerformanceScore {
    pub fn aggregate(&self) -> f32 {
        0.4 * self.completion_rate
            + 0.3 * self.quality_score
            + 0.2 * self.efficiency
            + 0.1 * self.collaboration_score
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeEvent {
    pub id: EventId,
    pub session_id: SessionId,
    pub task_id: Option<TaskId>,
    pub at: DateTime<Utc>,
    pub kind: RuntimeEventKind,
}

impl RuntimeEvent {
    pub fn new(session_id: SessionId, task_id: Option<TaskId>, kind: RuntimeEventKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            task_id,
            at: Utc::now(),
            kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuntimeEventKind {
    SessionCreated {
        objective: String,
    },
    PhaseChanged {
        phase: SessionPhase,
    },
    PlanCreated {
        node_count: usize,
    },
    TaskStatusChanged {
        status: TaskStatus,
    },
    ToolCallStarted {
        capability: CapabilityRef,
        input: serde_json::Value,
    },
    ToolCallFinished {
        output: serde_json::Value,
    },
    ApprovalRequested {
        reason: String,
    },
    ApprovalResolved {
        approved: bool,
        note: Option<String>,
    },
    Message {
        role: String,
        content: String,
    },
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_performance_score_with_readme_weights() {
        let score = PerformanceScore {
            completion_rate: 1.0,
            quality_score: 0.8,
            efficiency: 0.5,
            collaboration_score: 0.7,
        };

        assert!((score.aggregate() - 0.81).abs() < f32::EPSILON);
    }

    #[test]
    fn creates_default_company_departments() {
        let company = CompanyProfile::default();
        assert_eq!(company.departments.len(), 3);
        assert_eq!(company.departments[0].key, "research");
    }
}
