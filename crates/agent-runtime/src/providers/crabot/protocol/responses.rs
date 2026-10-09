use super::super::model_error::model_error;
use super::super::{
    config::HarnessConfig,
    turn::{Delta, Turn},
};
use super::contract::ModelProtocol;
use crate::tools::{ToolDefinition, ToolRegistry};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub(super) struct ResponsesProtocol;
impl ModelProtocol for ResponsesProtocol {
    fn image(&self, image: &crate::attachments::AttachmentImage) -> Value {
        json!({"type":"input_image","image_url":format!("data:{};base64,{}",image.media_type,image.data)})
    }
    fn tool(&self, d: &ToolDefinition) -> Value {
        json!({"type":"function","name":d.name(),"description":d.description(),"parameters":d.parameters(),"strict":false})
    }
    fn request(
        &self,
        http: &reqwest::Client,
        cfg: &HarnessConfig,
        history: &[Value],
        tools: &ToolRegistry,
    ) -> reqwest::RequestBuilder {
        let mut body = json!({"model":cfg.model,"instructions":cfg.system_prompt,"input":history,"tools":self.tools(tools),"stream":true,"store":false,"include":["reasoning.encrypted_content"],"max_output_tokens":cfg.max_tokens});
        if let Some(effort) = &cfg.deepseek_effort {
            body["reasoning"] = json!({"effort":effort});
        }
        let request = http.post(format!("{}/responses", cfg.base)).json(&body);
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
        if v["type"] == "response.output_text.delta" {
            if let Some(text) = v["delta"].as_str() {
                turn.append_text(text, delta);
            }
        }
        if let Some(text) = v["delta"].as_str().filter(|_| {
            matches!(
                v["type"].as_str().unwrap_or_default(),
                "response.reasoning_summary_text.delta" | "response.reasoning_text.delta"
            )
        }) {
            turn.append_reasoning(text, delta);
        }
        if v["type"] == "response.incomplete" {
            return Err(model_error(&v.to_string()));
        }
        if v["type"] == "response.completed" {
            turn.finish("stop");
            let output = v["response"]["output"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for (i, item) in output.iter().enumerate() {
                if item["type"] == "function_call" {
                    turn.calls_mut()
                        .insert(i, json!({"id":item["call_id"],"name":item["name"]}));
                    turn.arguments_mut()
                        .insert(i, item["arguments"].as_str().unwrap_or("{}").into());
                }
            }
            turn.set_output(output);
        }
        Ok(())
    }
    fn append_history(
        &self,
        history: &mut Vec<Value>,
        turn: &Turn,
        results: &BTreeMap<usize, String>,
    ) {
        history.extend_from_slice(turn.output());
        for (&index, result) in results {
            history.push(json!({"type":"function_call_output","call_id":turn.calls()[&index]["id"],"output":result}));
        }
    }
}
