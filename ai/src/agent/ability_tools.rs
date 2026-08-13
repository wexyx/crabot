use std::{collections::HashMap, sync::Arc};

use anyhow::Error;
use async_trait::async_trait;
use common::{biz_err, tool::ToolSchema};
use futures::lock::Mutex;

use crate::agent::{ability::AbilityFactory, carbot::Carbot, carbot_planner_tool, model::NodeInfo};

pub struct Factory {
    pub data: Mutex<HashMap<String, ToolSchema>>,
}

impl Factory {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(Default::default()),
        }
    }
}

#[async_trait]
impl AbilityFactory for Factory {
    async fn register(&self, tool: ToolSchema) {
        let mut data = self.data.lock().await;
        data.insert(tool.name.clone(), tool);
    }

    async fn tools(&self) -> Result<Vec<ToolSchema>, Error> {
        let data = self.data.lock().await;
        let schemas = data.values().cloned().collect();

        Ok(schemas)
    }

    async fn tool(&self, name: &str) -> Result<ToolSchema, Error> {
        let data = self.data.lock().await;
        let schema = data.get(name).ok_or(biz_err!("tool not found"))?;
        Ok(schema.clone())
    }

    async fn create_carbot(&self, parent: Arc<Carbot>, node: NodeInfo) -> Result<Carbot, Error> {
        let tool_delegate = carbot_planner_tool::ToolCaller::new(node);
        let carbot = Carbot::new(parent, Box::new(tool_delegate));
        Ok(carbot)
    }
}
