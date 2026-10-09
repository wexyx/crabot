use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeErrorCode {
    TokenInsufficient,
    ExecutionFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    TextDelta {
        text: String,
    },
    /// Model deliberation streamed for transparency. Never appended to the assistant
    /// answer; presentation layers fold it into the in-progress execution area.
    ReasoningDelta {
        text: String,
    },
    ToolStarted {
        id: String,
        name: String,
    },
    ToolFinished {
        id: String,
        output: String,
    },
    /// Opaque execution observations retained for continuation, not displayed as an answer.
    ContextCheckpoint {
        content: String,
    },
    Completed {
        text: String,
    },
    Failed {
        code: RuntimeErrorCode,
        message: String,
    },
}
impl RuntimeEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }
    /// Compatibility projection for callers that only understand text deltas.
    pub fn legacy_delta(&self) -> Option<String> {
        match self {
            Self::TextDelta { text } => Some(text.clone()),
            Self::ToolStarted { name, .. } => Some(format!("\n[工具：{name}]\n")),
            // Reasoning is deliberately absent: a text-only consumer must not surface it.
            _ => None,
        }
    }
    /// Progress mapping only. Gateways publish the returned result as their terminal event,
    /// so a dropped bounded-channel item cannot lose or duplicate completion.
    pub fn wire_progress(&self) -> Option<(&'static str, String)> {
        match self {
            Self::TextDelta { text } => Some(("agent.delta", text.clone())),
            Self::ReasoningDelta { text } => Some((
                "agent.reasoning",
                json!({"type": "reasoning_delta", "text": text}).to_string(),
            )),
            Self::ToolStarted { .. } => {
                Some(("agent.tool.started", serde_json::to_string(self).unwrap()))
            }
            Self::ToolFinished { .. } => {
                Some(("agent.tool.finished", serde_json::to_string(self).unwrap()))
            }
            Self::ContextCheckpoint { content } => Some(("agent.context", content.clone())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gateway_projection_cannot_duplicate_terminal() {
        assert!(
            RuntimeEvent::Completed {
                text: "done".into()
            }
            .wire_progress()
            .is_none()
        );
        assert!(
            RuntimeEvent::Failed {
                code: RuntimeErrorCode::ExecutionFailed,
                message: "failed".into()
            }
            .wire_progress()
            .is_none()
        );
        // Compare parsed fields; JSON object key order is not part of the contract.
        let (kind, payload) = RuntimeEvent::ReasoningDelta { text: "hmm".into() }
            .wire_progress()
            .unwrap();
        assert_eq!(kind, "agent.reasoning");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&payload).unwrap(),
            json!({"type":"reasoning_delta","text":"hmm"})
        );
        assert!(
            RuntimeEvent::ReasoningDelta { text: "hmm".into() }
                .legacy_delta()
                .is_none()
        );
        assert_eq!(
            RuntimeEvent::ToolStarted {
                id: "a".into(),
                name: "read".into()
            }
            .wire_progress()
            .unwrap()
            .0,
            "agent.tool.started"
        );
    }
}
