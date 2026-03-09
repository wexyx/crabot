use std::fmt::Display;

use anyhow::Error;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::schema::Schema;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ToolSchema {
    pub name: String,
    pub desc: String,
    pub input: Schema,
    pub output: Schema,
}

#[async_trait]
pub trait Tool: Send + Sync {
    async fn schema(&self) -> ToolSchema;
    async fn run(&self) -> Result<String, Error>;
}

impl From<ToolSchema> for Result<openai_api_rs::v1::chat_completion::Tool, Error> {
    fn from(value: ToolSchema) -> Self {
        let params: Result<openai_api_rs::v1::types::FunctionParameters, Error> = value.input.into();
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
        let tool : Result<openai_api_rs::v1::chat_completion::Tool, Error> = self.clone().into();
        let data = tool.map(|v|serde_json::to_string_pretty(&v).unwrap()).unwrap_or_else(|e|e.to_string());
        f.write_str(data.as_str())
    }
}
