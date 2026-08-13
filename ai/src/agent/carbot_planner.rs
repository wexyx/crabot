use anyhow::Error;
use async_recursion::async_recursion;
use async_trait::async_trait;
use common::task;
use futures::lock::Mutex;
use openai_api_rs::v1::chat_completion::{ChatCompletionMessage, Content, MessageRole, Tool};
use std::{collections::HashMap, sync::Arc};

use crate::{
    agent::{
        ability::AbilityFactory, agent::Agent, common::request_for_plan, constant::PLANNER_PROMPT,
        model::NodeInfo,
    },
    tools,
};

use crate::agent::carbot::CarbotDelegate;

pub struct Planner {
    node: Mutex<NodeInfo>,
    ability: Arc<Box<dyn AbilityFactory>>,
    results: Mutex<Vec<String>>,
}

impl Planner {
    pub fn new(node: NodeInfo, ability: Arc<Box<dyn AbilityFactory>>) -> Self {
        Self {
            node: Mutex::new(node),
            ability,
            results: Mutex::new(Default::default()),
        }
    }
}

#[async_trait]
impl CarbotDelegate for Planner {
    async fn run(&self, carbot: Arc<crate::agent::carbot::Carbot>) -> Result<String, Error> {
        for _ in 0..20 {
            let finish = self.do_plan(carbot.clone()).await?;
            if finish {
                break;
            }
        }

        let results = self.get_results().await;
        let mut result = String::new();
        for r in results {
            result.push_str(r.as_str());
            result.push_str("\n");
        }

        Ok(result)
    }
}

impl Planner {
    #[async_recursion]
    async fn do_plan(&self, carbot: Arc<crate::agent::carbot::Carbot>) -> Result<bool, Error> {
        let node = self.node().await?;
        let requirements = carbot.find_requirements(node.requirements).await?;

        let plan_info = request_for_plan(self.messages(requirements).await?).await?;
        if plan_info.is_none() {
            return Ok(true);
        }

        let plan_info = plan_info.unwrap_or_default();
        if plan_info.finish {
            if let Some(plan_result) = plan_info.result {
                self.append_result(plan_result).await;
            }

            return Ok(true);
        }

        let mut futures = vec![];
        if let Some(nodes) = plan_info.nodes {
            for node in nodes {
                let child = self.ability.create_carbot(carbot.clone(), node).await?;
                let child = Arc::new(child);
                futures.push(task::spawn_result(async move { child.start().await }));
            }
        }

        let mut result = String::new();
        for future in futures {
            let data = future.await?;
            if result.len() > 0 {
                result.push_str("\n");
            }

            if data.len() > 0 {
                result.push_str(data.as_str());
            }
        }

        Ok(false)
    }

    async fn tools(&self) -> Result<Vec<Tool>, Error> {
        let mut tools = Vec::new();
        for schema in tools::list().await? {
            let tool: Result<Tool, Error> = schema.into();
            tools.push(tool?);
        }

        Ok(tools)
    }

    async fn node(&self) -> Result<NodeInfo, Error> {
        let node = self.node.lock().await;
        Ok(node.clone())
    }
}

impl Planner {
    async fn append_result(&self, result: String) {
        let mut results = self.results.lock().await;
        results.push(result);
    }

    async fn get_results(&self) -> Vec<String> {
        let results = self.results.lock().await;
        results.clone()
    }

    async fn messages(
        &self,
        params: HashMap<String, String>,
    ) -> Result<Vec<ChatCompletionMessage>, Error> {
        let mut data = vec![
            ChatCompletionMessage {
                role: MessageRole::system,
                content: Content::Text(PLANNER_PROMPT.to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatCompletionMessage {
                role: MessageRole::assistant,
                content: Content::Text(self.node.lock().await.goal.clone()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let tools = self.tools().await?;
        if tools.len() > 0 {
            for tool in tools {
                data.push(ChatCompletionMessage {
                    role: MessageRole::assistant,
                    content: Content::Text(serde_json::to_string(&tool).unwrap_or_default()),
                    name: Some("节点功能信息".to_string()),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }

        let results = self.get_results().await;
        if results.len() > 0 {
            for result in results {
                data.push(ChatCompletionMessage {
                    role: MessageRole::tool,
                    content: Content::Text(result),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }

        if params.len() > 0 {
            data.push(ChatCompletionMessage {
                role: MessageRole::assistant,
                content: Content::Text(serde_json::to_string(&params).unwrap_or_default()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            });
        }

        Ok(data)
    }
}
