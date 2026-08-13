use std::fmt::Display;

use crate::schema::Schema;
use anyhow::Error;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ToolSchema {
    pub name: String,
    pub desc: String,
    pub input: Schema,
    pub output: Schema,
    #[serde(default)]
    pub metadata: CapabilityMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CapabilityMetadata {
    pub source: CapabilitySourceKind,
    pub approval: ApprovalRequirement,
    pub tags: Vec<String>,
}

impl Default for CapabilityMetadata {
    fn default() -> Self {
        Self {
            source: CapabilitySourceKind::Builtin,
            approval: ApprovalRequirement::Never,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum CapabilitySourceKind {
    #[default]
    Builtin,
    Plugin,
    Mcp,
    Skill,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequirement {
    #[default]
    Never,
    OnSideEffect,
    Always,
}

impl ApprovalRequirement {
    pub fn needs_gate(self) -> bool {
        matches!(self, Self::OnSideEffect | Self::Always)
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    async fn schema(&self) -> ToolSchema;
    async fn run(&self) -> Result<String, Error>;
}

impl From<ToolSchema> for Result<openai_api_rs::v1::chat_completion::Tool, Error> {
    fn from(value: ToolSchema) -> Self {
        let params: Result<openai_api_rs::v1::types::FunctionParameters, Error> =
            value.input.into();
        let tool = openai_api_rs::v1::chat_completion::Tool {
            r#type: openai_api_rs::v1::chat_completion::ToolType::Function,
            function: openai_api_rs::v1::types::Function {
                name: value.name,
                description: Some(value.desc),
                parameters: params?,
            },
        };

        Ok(tool)
    }
}

impl Display for ToolSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tool: Result<openai_api_rs::v1::chat_completion::Tool, Error> = self.clone().into();
        let data = tool
            .map(|v| serde_json::to_string_pretty(&v).unwrap())
            .unwrap_or_else(|e| e.to_string());
        f.write_str(data.as_str())
    }
}
