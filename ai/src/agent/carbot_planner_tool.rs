use anyhow::{Error, Ok};
use openai_api_rs::v1::chat_completion::{ChatCompletionMessage, chat_completion::ChatCompletionRequest};
use std::{collections::HashMap, hash::Hash, sync::Arc, time::Duration};
use async_trait::async_trait;
use futures::lock::Mutex;
use openai_api_rs::v1::{chat_completion::{ChatCompletionChoice, Content, MessageRole, ToolCall}};

use crate::{agent::{ability::AbilityFactory, agent::Agent, carbot::Carbot, common::{load_tool_calling_info}, constant::{PLANNER_PROMPT, TOOL_PROMPT}, model::{NodeInfo, PlanInfo}}, client::openai, tools};

use crate::agent::{carbot::CarbotDelegate};

pub struct ToolCaller {
    node: Mutex<NodeInfo>,
}

impl ToolCaller {
    pub fn new(node: NodeInfo) -> Self {
        Self { 
            node: Mutex::new(node),
        }
    }
}

#[async_trait]
impl CarbotDelegate for ToolCaller {
    async fn run(&self, carbot: Arc<crate::agent::carbot::Carbot>) -> Result<String, Error> {
        let tool = self.tool().await?;
        let node = self.node().await?;
        log::error!("CarbotDelegate find requirements: {:?}", node.requirements);
        let mut requirements = carbot.find_requirements(node.requirements).await?;
        let toolcall_info = load_tool_calling_info(self.messages(requirements.clone()).await, tool.clone()).await?;

        if toolcall_info.is_none() {
            return Ok("".to_string());
        }

        let toolcall_info = toolcall_info.unwrap_or_default();
        if let Some(result) = toolcall_info.result {
            return Ok(result);
        }

        let mut tool_params = toolcall_info.params.unwrap_or("{}".to_string());
        let requirement_values = carbot.find_requirements(toolcall_info.requirements).await?;
        if requirement_values.len() > 0 {
            for (k, v) in requirement_values {
                requirements.insert(k, v);
            }

            let toolcall_info = load_tool_calling_info(self.messages(requirements).await, tool.clone()).await?;
            if toolcall_info.is_none() {
                return Ok("".to_string());
            }

            let toolcall_info = toolcall_info.unwrap_or_default();
            if let Some(params) = toolcall_info.params {
                tool_params = params;
            }
        }

        self.call_tool(&tool.function.name, tool_params).await
    }
}

impl ToolCaller {
    async fn tool(&self) -> Result<openai_api_rs::v1::chat_completion::Tool, Error> {
        let schema = tools::schema(&self.node.lock().await.key).await?;
        let tool: Result<openai_api_rs::v1::chat_completion::Tool, Error> = schema.into();
        tool
    }

    async fn node(&self) -> Result<NodeInfo, Error> {
        let node = self.node.lock().await;
        Ok(node.clone())
    }
}

impl ToolCaller {
    async fn messages(&self, params: HashMap<String, String>) -> Vec<ChatCompletionMessage> {
        let mut data = vec![ChatCompletionMessage {
            role: MessageRole::system,
            content: Content::Text(TOOL_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }, ChatCompletionMessage {
            role: MessageRole::assistant,
            content: Content::Text(self.node.lock().await.goal.clone()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        if params.len() > 0 {
            data.push(ChatCompletionMessage {
            role: MessageRole::assistant,
            content: Content::Text(serde_json::to_string(&params).unwrap_or_default()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
        }

        data
    }
 
    async fn call_tool(&self, name: &str, params: String) -> Result<String, Error> {
        let result = tools::call(&name, &params).await?;
        return Ok(result)
    }
}