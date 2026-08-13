use crate::{
    agent::model::{PlanInfo, ToolCallInfo},
    client::openai,
};
use anyhow::Error;
use openai_api_rs::v1::chat_completion::{
    ChatCompletionMessage, Tool, chat_completion::ChatCompletionRequest,
};

pub async fn request_for_plan(
    messages: Vec<ChatCompletionMessage>,
) -> Result<Option<PlanInfo>, Error> {
    // 每次都是do_plan，传递上下文，判断是否已经ready
    let req = ChatCompletionRequest {
        model: "openai/gpt-5.2".to_string(),
        messages: messages,
        response_format: None,
        user: None,
        tools: None,
        temperature: None,
        top_p: None,
        n: None,
        stop: None,
        max_tokens: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        seed: None,
        parallel_tool_calls: None,
        tool_choice: None,
        reasoning: None,
        transforms: None,
    };

    log::error!(
        "openai_api_rs req: {}",
        serde_json::to_string(&req).unwrap_or_default()
    );
    // todo: client和tool一样，直接全局路由
    let client = openai::OpenAI::new(
        Some("https://openrouter.ai/api/v1".to_string()),
        "".to_string(),
    )?;
    let resp: openai_api_rs::v1::chat_completion::chat_completion::ChatCompletionResponse =
        client.chat(req).await?;
    log::error!(
        "openai_api_rs resp: {}",
        serde_json::to_string(&resp).unwrap_or_default()
    );

    let result = resp.choices.get(0);
    if let Some(result) = result {
        if let Some(content) = result.clone().message.content {
            let plan = serde_json::from_str(&content)?;
            return Ok(Some(plan));
        }
    }

    Ok(None)
}

pub async fn load_tool_calling_info(
    messages: Vec<ChatCompletionMessage>,
    tool: Tool,
) -> Result<Option<ToolCallInfo>, Error> {
    // 每次都是do_plan，传递上下文，判断是否已经ready
    let req = ChatCompletionRequest {
        model: "openai/gpt-5.2".to_string(),
        messages: messages,
        response_format: None,
        user: None,
        tools: Some(vec![tool]),
        temperature: None,
        top_p: None,
        n: None,
        stop: None,
        max_tokens: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        seed: None,
        parallel_tool_calls: None,
        tool_choice: None,
        reasoning: None,
        transforms: None,
    };

    // todo: client和tool一样，直接全局路由
    log::error!(
        "openai_api_rs tool req: {}",
        serde_json::to_string(&req).unwrap_or_default()
    );
    let client = openai::OpenAI::new(
        Some("https://openrouter.ai/api/v1".to_string()),
        "".to_string(),
    )?;
    let resp: openai_api_rs::v1::chat_completion::chat_completion::ChatCompletionResponse =
        client.chat(req).await?;
    log::error!(
        "openai_api_rs tool resp: {}",
        serde_json::to_string(&resp).unwrap_or_default()
    );
    let result = resp.choices.get(0);
    if let Some(result) = result {
        if let Some(content) = result.clone().message.content {
            let info = serde_json::from_str(&content)?;
            return Ok(Some(info));
        }
    }

    Ok(None)
}
