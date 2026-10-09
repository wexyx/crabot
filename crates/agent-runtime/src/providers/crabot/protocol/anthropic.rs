use super::super::{
    config::HarnessConfig,
    turn::{Delta, Turn},
};
use super::contract::ModelProtocol;
use crate::tools::{ToolDefinition, ToolRegistry};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub(super) struct AnthropicProtocol {
    output_limit: tokio::sync::OnceCell<u64>,
}
impl AnthropicProtocol {
    pub(super) fn new() -> Self {
        Self {
            output_limit: tokio::sync::OnceCell::new(),
        }
    }
}
impl ModelProtocol for AnthropicProtocol {
    fn prepare_request<'a>(
        &'a self,
        http: &'a reqwest::Client,
        cfg: &'a HarnessConfig,
        history: &'a [Value],
        tools: &'a ToolRegistry,
    ) -> futures_util::future::BoxFuture<'a, Result<reqwest::RequestBuilder, String>> {
        Box::pin(async move {
            let mut resolved = cfg.clone();
            if resolved.max_tokens.is_none() {
                let limit = self
                    .output_limit
                    .get_or_try_init(|| super::anthropic_limits::discover(http, cfg))
                    .await?;
                resolved.max_tokens = Some(*limit);
            }
            Ok(self.request(http, &resolved, history, tools))
        })
    }
    fn image(&self, image: &crate::attachments::AttachmentImage) -> Value {
        json!({"type":"image","source":{"type":"base64","media_type":image.media_type,"data":image.data}})
    }
    fn tool(&self, d: &ToolDefinition) -> Value {
        json!({"name":d.name(),"description":d.description(),"input_schema":d.parameters()})
    }
    fn request(
        &self,
        http: &reqwest::Client,
        cfg: &HarnessConfig,
        history: &[Value],
        tools: &ToolRegistry,
    ) -> reqwest::RequestBuilder {
        let mut body = json!({"model":cfg.model,"system":cfg.system_prompt,"messages":history,"tools":self.tools(tools),"stream":true});
        if let Some(limit) = cfg.max_tokens {
            body["max_tokens"] = json!(limit);
        }
        http.post(format!("{}/messages", cfg.base))
            .json(&body)
            .header("x-api-key", &cfg.key)
            .header("anthropic-version", "2023-06-01")
    }
    fn consume(
        &self,
        turn: &mut Turn,
        v: Value,
        delta: &mut dyn FnMut(Delta),
    ) -> Result<(), String> {
        if v["delta"]["type"] == "text_delta" {
            if let Some(text) = v["delta"]["text"].as_str() {
                turn.append_text(text, delta);
            }
        }
        if v["delta"]["type"] == "thinking_delta" {
            if let Some(text) = v["delta"]["thinking"].as_str() {
                turn.append_reasoning(text, delta);
            }
        }
        let i = v["index"].as_u64().unwrap_or(0) as usize;
        if v["type"] == "content_block_start" && v["content_block"]["type"] == "tool_use" {
            turn.calls_mut().insert(
                i,
                json!({"id":v["content_block"]["id"],"name":v["content_block"]["name"]}),
            );
            turn.arguments_mut().insert(i, String::new());
        }
        if let Some(part) = v["delta"]["partial_json"].as_str() {
            turn.arguments_mut().entry(i).or_default().push_str(part);
        }
        if let Some(reason) = v["delta"]["stop_reason"].as_str() {
            turn.finish(reason);
        }
        Ok(())
    }
    fn append_history(
        &self,
        history: &mut Vec<Value>,
        turn: &Turn,
        results: &BTreeMap<usize, String>,
    ) {
        let mut calls = Vec::new();
        let mut outputs = Vec::new();
        if !turn.text().is_empty() {
            calls.push(json!({"type":"text","text":turn.text()}));
        }
        for (&index, result) in results {
            let call = &turn.calls()[&index];
            // Run has validated JSON arguments before execution.
            let args: Value =
                serde_json::from_str(turn.arguments(index)).expect("validated tool arguments");
            calls.push(json!({"type":"tool_use","id":call["id"],"name":call["name"],"input":args}));
            outputs.push(json!({"type":"tool_result","tool_use_id":call["id"],"content":result}));
        }
        history.push(json!({"role":"assistant","content":calls}));
        history.push(json!({"role":"user","content":outputs}));
    }
}
