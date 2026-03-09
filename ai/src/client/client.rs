use std::pin::Pin;

use anyhow::Error;
use async_trait::async_trait;
use futures::Stream;
use openai_api_rs::v1::{chat_completion::{chat_completion::{ChatCompletionRequest, ChatCompletionResponse}, chat_completion_stream::ChatCompletionStreamResponse}, embedding::{EmbeddingRequest, EmbeddingResponse}};

#[async_trait]
pub trait Client: Send + Sync {
    async fn chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse, Error>;
    async fn stream_chat(&self, req: ChatCompletionRequest) -> Result<Pin<Box<dyn Stream<Item = ChatCompletionStreamResponse> + Send>>, Error>;
    async fn embedding(&self, req: EmbeddingRequest) -> Result<EmbeddingResponse, Error>;
}