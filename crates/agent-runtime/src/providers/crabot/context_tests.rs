use super::{
    client::ModelClient,
    config::{HarnessConfig, ModelApi},
};
use crate::{RuntimeEvent, context::ContextBudget, tools::ToolRegistry};
use serde_json::json;

#[tokio::test]
async fn compaction_removes_native_tool_pairs_together_for_every_protocol() {
    for api in [ModelApi::Chat, ModelApi::Responses, ModelApi::Anthropic] {
        let client = ModelClient::new(HarnessConfig {
            environment: Default::default(),
            system_prompt: crate::config::default_crabot_system_prompt().into(),
            context: ContextBudget::from_lookup(|key| {
                (key == "CONTEXT_MAX_TOKENS").then(|| "8192".into())
            })
            .unwrap(),
            api,
            base: "http://127.0.0.1/v1".into(),
            key: "fixture".into(),
            model: "fixture".into(),
            max_tokens: Some(1024),
            deepseek_effort: None,
            root: ".".into(),
        })
        .unwrap();
        let mut history = vec![json!({"role":"user","content":"Complete the current task"})];
        match api {
            ModelApi::Chat => history.extend([json!({"role":"assistant","tool_calls":[{"id":"a","type":"function","function":{"name":"echo","arguments":"{}"}}]}),json!({"role":"tool","tool_call_id":"a","content":"中文".repeat(10000)})]),
            ModelApi::Responses => history.extend([json!({"type":"function_call","call_id":"a","name":"echo","arguments":"{}"}),json!({"type":"function_call_output","call_id":"a","output":"中文".repeat(10000)})]),
            ModelApi::Anthropic => history.extend([json!({"role":"assistant","content":[{"type":"tool_use","id":"a","name":"echo","input":{}}]}),json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"a","content":"中文".repeat(10000)}]})]),
        }
        let mut events = Vec::new();
        client
            .prepare_context(&mut history, &ToolRegistry::new(), &mut |e| events.push(e))
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["role"], "user");
        assert!(
            history[0]["content"]
                .as_str()
                .unwrap()
                .ends_with("Complete the current task")
        );
        assert!(matches!(events[0], RuntimeEvent::ContextCheckpoint { .. }));
    }
}
