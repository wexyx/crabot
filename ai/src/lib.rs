use std::sync::Arc;

use anyhow::{Error, Ok};
use openai_api_rs::v1::{chat_completion::{ChatCompletionMessage, Content, MessageRole}, embedding::EmbeddingRequest};

use crate::agent::{ability::AbilityFactory, ability_tools, agent::Agent, carbot_planner, model::NodeInfo};

pub mod tools;
pub mod mcp;
pub mod skill;
pub mod memory;
pub mod client;
pub mod agent;

pub fn init() {
    common::registry::register();
}

pub async fn debug() -> Result<(), Error> {
    Ok(())
}

async fn embedding(data: &str) -> Result<Vec<u8>, Error> {
    let client = client::openai::OpenAI::new(Some("https://openrouter.ai/api/v1".to_string()), "".to_string())?;
    let emb = client.embedding(EmbeddingRequest { 
        model: "openai/text-embedding-3-small".to_string(), 
        input: vec![data.to_string()], 
        encoding_format: None, 
        dimensions: None, 
        user: None,
    }).await?;

    let r = emb.data.get(0).ok_or(anyhow::Error::msg("notfound"))?;
    Ok(vec_f32_to_bytes(&r.embedding))
}

pub async fn demo() -> Result<(), Error> {
    let ability = ability_tools::Factory::new();
    for tool in tools::list().await? {
        ability.register(tool).await;
    }

    let planner = carbot_planner::Planner::new(NodeInfo{
        key: "root_node".to_string(),
        goal: "打印一个hello_world".to_string(),
        result: None,
        requirements: vec![],
    }, Arc::new(Box::new(ability)));
    let carbot = agent::carbot::Carbot::root(Box::new(planner));
    let carbot = Arc::new(carbot);
    carbot.start().await?;

    Ok(())
}


/// 将 Vec<f32> 转成 Vec<u8>，按内存原样存储
pub fn vec_f32_to_bytes(embedding: &Vec<f32>) -> Vec<u8> {
    let len = embedding.len() * std::mem::size_of::<f32>();
    let mut bytes = vec![0u8; len];

    // 安全地按内存复制
    unsafe {
        std::ptr::copy_nonoverlapping(
            embedding.as_ptr() as *const u8,
            bytes.as_mut_ptr(),
            len,
        );
    }

    bytes
}