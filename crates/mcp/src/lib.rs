use crabot_domain::{CapabilityRef, CapabilitySource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerDefinition {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub tools: Vec<McpToolDefinition>,
}

impl McpServerDefinition {
    pub fn capability_refs(&self) -> Vec<CapabilityRef> {
        self.tools
            .iter()
            .map(|tool| {
                CapabilityRef::new(
                    CapabilitySource::Mcp,
                    format!("{}:{}", self.name, tool.name),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolCall {
    pub server: String,
    pub tool: String,
    pub input: serde_json::Value,
}
