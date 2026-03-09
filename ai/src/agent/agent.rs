use std::{collections::HashMap, sync::Arc};
use anyhow::Error;
use async_trait::async_trait;
use crate::agent::model::Requirement;

#[async_trait]
pub trait Agent: Send + Sync {
    // 向模型发送消息
    async fn start(self: Arc<Self>) -> Result<String, Error>;

    // 依赖解析，由最外层的top_agent根据全局上下文解析一次，解析不了的丢给用户，用户输入后继续，key可以作为本地缓存
    async fn find_requirements(&self, requirements: Vec<Requirement>)-> Result<HashMap<String, String>, Error>;
}
