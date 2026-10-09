use super::super::{
    config::HarnessConfig,
    turn::{Delta, Turn},
};
use super::contract::ModelProtocol;
use crate::tools::{ToolDefinition, ToolRegistry};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub(super) struct ChatProtocol;
impl ModelProtocol for ChatProtocol {
    fn image(&self, image: &crate::attachments::AttachmentImage) -> Value {
        json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}",image.media_type,image.data)}})
    }
    fn tool(&self, d: &ToolDefinition) -> Value {
        json!({"type":"function","function":{"name":d.name(),"description":d.description(),"parameters":d.parameters()}})
    }
    fn request(
        &self,
        http: &reqwest::Client,
        cfg: &HarnessConfig,
        history: &[Value],
        tools: &ToolRegistry,
    ) -> reqwest::RequestBuilder {
        let mut messages = vec![json!({"role":"system","content":cfg.system_prompt})];
        messages.extend_from_slice(history);
        let mut body = json!({"model":cfg.model,"messages":messages,"tools":self.tools(tools),"stream":true,"max_tokens":cfg.max_tokens});
        if let Some(effort) = &cfg.deepseek_effort {
            body["thinking"] =
                json!({"type": if effort == "none" { "disabled" } else { "enabled" }});
            body["reasoning_effort"] = json!(effort);
        }
        let request = http
            .post(format!("{}/chat/completions", cfg.base))
            .json(&body);
        if cfg.key.is_empty() {
            request
        } else {
            request.bearer_auth(&cfg.key)
        }
    }
    fn consume(
        &self,
        turn: &mut Turn,
        v: Value,
        delta: &mut dyn FnMut(Delta),
    ) -> Result<(), String> {
        if let Some(text) = v
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            turn.append_text(text, delta);
        }
        for part in v
            .pointer("/choices/0/delta/reasoning_content")
            .into_iter()
            .chain(v.pointer("/choices/0/delta/reasoning"))
            .filter_map(Value::as_str)
        {
            turn.append_reasoning(part, delta);
        }
        if let Some(calls) = v
            .pointer("/choices/0/delta/tool_calls")
            .and_then(Value::as_array)
        {
            for call in calls {
                let i = call["index"].as_u64().unwrap_or(0) as usize;
                let current = turn
                    .calls_mut()
                    .entry(i)
                    .or_insert(json!({"id":"","name":""}));
                for (source, target) in [("/id", "id"), ("/function/name", "name")] {
                    if let Some(part) = call.pointer(source).and_then(Value::as_str) {
                        current[target] = json!(format!(
                            "{}{}",
                            current[target].as_str().unwrap_or_default(),
                            part
                        ));
                    }
                }
                if let Some(extra) = call.get("extra_content") {
                    current["extra_content"] = extra.clone();
                }
                if let Some(part) = call["function"]["arguments"].as_str() {
                    turn.arguments_mut().entry(i).or_default().push_str(part);
                }
            }
        }
        if let Some(reason) = v
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
        {
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
        for (&index, result) in results {
            let call = &turn.calls()[&index];
            let mut native = json!({"id":call["id"],"type":"function","function":{"name":call["name"],"arguments":turn.arguments(index)}});
            if let Some(extra) = call.get("extra_content") {
                native["extra_content"] = extra.clone();
            }
            calls.push(native);
            outputs.push(json!({"role":"tool","tool_call_id":call["id"],"content":result}));
        }
        let mut assistant = json!({"role":"assistant","content":turn.text(),"tool_calls":calls});
        if !turn.reasoning().is_empty() {
            assistant["reasoning_content"] = json!(turn.reasoning());
        }
        history.push(assistant);
        history.extend(outputs);
    }
}
