use std::pin::Pin;

use anyhow::{Error, Ok};
use async_trait::async_trait;
use futures::Stream;
use openai_api_rs::v1::{api::OpenAIClient, chat_completion::{chat_completion::{ChatCompletionRequest, ChatCompletionResponse}, chat_completion_stream::ChatCompletionStreamResponse}, embedding::{EmbeddingRequest, EmbeddingResponse}};

use crate::client::client::Client;

pub struct OpenAI {
    openai_client: OpenAIClient
}

impl OpenAI {
    pub fn new(endpoint: Option<String>, api_key: String) -> Result<Box<dyn Client>, Error> {
        let openai_client = OpenAIClient::builder()
            .with_api_key(api_key)
            .with_endpoint(endpoint.unwrap_or("https://api.openai.com/v1".to_string()))
            .build()
            .map_err(|e|anyhow::Error::msg(format!("{}", e)))?;

        let c = Self {
            openai_client,
        };

        return Ok(Box::new(c));
    }
}

#[async_trait]
impl Client for OpenAI {
    async fn chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse, Error> {
        let result = self.openai_client.chat_completion(req).await?;
        Ok(result.inner)
    }

    async fn stream_chat(&self, req: ChatCompletionRequest) -> Result<Pin<Box<dyn Stream<Item = ChatCompletionStreamResponse> + Send>>, Error> {
        let result = self.openai_client.chat_completion_stream(req.into()).await;
        result.map(|v|Box::pin(v) as Pin<Box<dyn Stream<Item = ChatCompletionStreamResponse> + Send>>).map_err(Into::into)
    }

    async fn embedding(&self, req: EmbeddingRequest) -> Result<EmbeddingResponse, Error> {
        let result = self.openai_client.embedding(req).await?;
        Ok(result.inner)
    }
}