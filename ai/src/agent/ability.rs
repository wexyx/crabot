use std::sync::Arc;

use anyhow::Error;
use async_trait::async_trait;
use common::tool::ToolSchema;

use crate::agent::{carbot::Carbot, model::NodeInfo};

/// 这里的tool可以是function_calling也可以是mcp的接口，也可以是mcp，实际上就是进行分类
#[async_trait]
pub trait AbilityFactory: Send + Sync {
    // 注册新的工具进去
    async fn register(&self, tool: ToolSchema);
    // 所有的工具
    async fn tools(&self) -> Result<Vec<ToolSchema>, Error>;
    // 获取指定的工具信息
    async fn tool(&self, name: &str) -> Result<ToolSchema, Error>;
    // 获取指定的工具执行的agent
    async fn create_carbot(&self, parent: Arc<Carbot>, node: NodeInfo) -> Result<Carbot, Error>;
}
