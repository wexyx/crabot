#![cfg(test)]

#[test]
fn protocol_factory_formats_tools_and_authentication() {
    use crate::tools::{ToolDefinition, ToolRegistry};
    let definition = ToolDefinition::new("echo", "Echo", json!({"type":"object"}));
    for (api, path) in [
        (Api::Chat, "/v1/chat/completions"),
        (Api::Responses, "/v1/responses"),
        (Api::Anthropic, "/v1/messages"),
    ] {
        let protocol = ProtocolFactory::create(api);
        let cfg = Config {
            environment: Default::default(),
            context: Default::default(),
            system_prompt: crate::config::default_crabot_system_prompt().into(),
            api,
            base: "http://localhost/v1".into(),
            key: "test-secret".into(),
            model: "fixture".into(),
            max_tokens: Some(256),
            deepseek_effort: None,
            root: PathBuf::from("."),
        };
        let request = protocol
            .request(&reqwest::Client::new(), &cfg, &[], &ToolRegistry::new())
            .build()
            .unwrap();
        assert_eq!(request.url().path(), path);
        let tool = protocol.tool(&definition);
        if api == Api::Anthropic {
            assert_eq!(request.headers()["x-api-key"], "test-secret");
            assert_eq!(request.headers()["anthropic-version"], "2023-06-01");
            assert!(!request.headers().contains_key("authorization"));
            assert_eq!(tool["input_schema"], definition.parameters().clone());
        } else {
            assert_eq!(request.headers()["authorization"], "Bearer test-secret");
            assert!(!request.headers().contains_key("x-api-key"));
            if api == Api::Chat {
                assert_eq!(tool["function"]["name"], "echo");
            } else {
                assert_eq!(tool["name"], "echo");
                assert_eq!(tool["strict"], false);
            }
        }
    }
}

#[test]
fn turn_rejects_truncation_limits_and_excess_calls() {
    let mut turn = Turn::new();
    assert!(turn.validate().unwrap_err().contains("before completion"));
    turn.finish("length");
    assert!(turn.validate().unwrap_err().contains("TOKEN_INSUFFICIENT"));
    turn.finish("stop");
    for i in 0..17 {
        turn.calls_mut()
            .insert(i, json!({"id":format!("c{i}"),"name":"echo"}));
    }
    assert!(turn.validate().unwrap_err().contains("16 tool calls"));
}

#[test]
fn adapters_preserve_reasoning_and_vendor_metadata_in_history() {
    let chat = ProtocolFactory::create(Api::Chat);
    let mut turn = Turn::new();
    chat.consume(&mut turn,json!({"choices":[{"delta":{"reasoning_content":"reasoning","tool_calls":[{"index":0,"id":"call","function":{"name":"echo","arguments":"{}"},"extra_content":{"signature":"opaque"}}]},"finish_reason":"tool_calls"}]}),&mut |_| {}).unwrap();
    let mut history = Vec::new();
    chat.append_history(&mut history, &turn, &BTreeMap::from([(0, "ok".into())]));
    assert_eq!(history[0]["reasoning_content"], "reasoning");
    assert_eq!(
        history[0]["tool_calls"][0]["extra_content"]["signature"],
        "opaque"
    );
    let responses = ProtocolFactory::create(Api::Responses);
    let mut turn = Turn::new();
    let output = json!([{"type":"reasoning","encrypted_content":"opaque"},{"type":"function_call","call_id":"call","name":"echo","arguments":"{}"}]);
    responses
        .consume(
            &mut turn,
            json!({"type":"response.completed","response":{"output":output}}),
            &mut |_| {},
        )
        .unwrap();
    let mut history = Vec::new();
    responses.append_history(&mut history, &turn, &BTreeMap::from([(1, "ok".into())]));
    assert_eq!(history[0]["encrypted_content"], "opaque");
    assert_eq!(history[2]["call_id"], "call");
    assert!(responses.consume(&mut Turn::new(),json!({"type":"response.incomplete","response":{"incomplete_details":{"reason":"max_output_tokens"}}}),&mut |_|{}).unwrap_err().contains("TOKEN_INSUFFICIENT"));
}

use super::{
    config::{HarnessConfig as Config, ModelApi as Api},
    protocol::ProtocolFactory,
    turn::Turn,
};
use crate::RuntimeEvent;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn all_three_protocols_complete_twenty_tool_roundtrips() {
    for (tool_name, arguments, expected) in [
        ("find", r#"{"target":"tool","name":"shell"}"#, "shell"),
        (
            "find",
            r#"{"target":"skill","id":"demo"}"#,
            "shared skill instructions",
        ),
    ] {
        for api in [Api::Chat, Api::Responses, Api::Anthropic] {
            let count = Arc::new(AtomicUsize::new(0));
            let counter = count.clone();
            let app = axum::Router::new().fallback(axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let counter = counter.clone();
                async move {
                    let turn = counter.fetch_add(1, Ordering::SeqCst);
                    let chunks = if turn < 20 {
                        match api {
                            Api::Chat => vec![json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call1","type":"function","function":{"name":tool_name,"arguments":arguments}}]},"finish_reason":"tool_calls"}]})],
                            Api::Responses => vec![json!({"type":"response.completed","response":{"output":[{"type":"function_call","call_id":"call1","name":tool_name,"arguments":arguments}]}})],
                            Api::Anthropic => vec![json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call1","name":tool_name,"input":{}}}),json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":arguments}}),json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}})],
                        }
                    } else {
                        let result = match api {
                            Api::Chat => &body["messages"][2]["content"],
                            Api::Responses => &body["input"][2]["output"],
                            Api::Anthropic => &body["messages"][2]["content"][0]["content"],
                        };
                        // Chat has an extra system message before user/assistant/tool.
                        let result = if api == Api::Chat { &body["messages"][3]["content"] } else { result };
                        assert!(result.as_str().unwrap().contains(expected));
                        match api {
                            Api::Chat => vec![json!({"choices":[{"delta":{"content":"verified"},"finish_reason":"stop"}]})],
                            Api::Responses => vec![json!({"type":"response.output_text.delta","delta":"verified"}),json!({"type":"response.completed","response":{"output":[]}})],
                            Api::Anthropic => vec![json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"verified"}}),json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}})],
                        }
                    };
                    let stream = chunks.into_iter().map(|v| format!("data: {v}\n\n")).collect::<String>();
                    ([("content-type", "text/event-stream")], stream)
                }
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let cfg = Config {
                environment: Default::default(),
                context: Default::default(),
                system_prompt: crate::config::default_crabot_system_prompt().into(),
                api,
                base: format!("http://{address}"),
                key: "fixture".into(),
                model: "fixture".into(),
                max_tokens: Some(256),
                deepseek_effort: None,
                root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            };
            let mut output = Vec::new();
            let runtime = crate::RuntimeFactory::from_config_with_skills(
                crate::config::RuntimeConfig::Crabot(cfg),
                crate::skills::SkillCatalog::new(vec![
                    crate::skills::SkillDefinition::new(
                        "demo".into(),
                        "demo".into(),
                        BTreeMap::from([("SKILL.md".into(), "shared skill instructions".into())]),
                        true,
                        false,
                    )
                    .unwrap(),
                ])
                .unwrap(),
                crate::skills::ExecutionPolicy::new("offline".into()).unwrap(),
            )
            .unwrap();
            let result = runtime
                .run_events("read manifest", &mut |event| output.push(event))
                .await;
            server.abort();
            assert_eq!(result.unwrap(), "verified");
            assert!(
                output
                    .iter()
                    .any(|e| matches!(e,RuntimeEvent::ToolStarted{name,..} if name==tool_name))
            );
            assert!(output.iter().any(|e|matches!(e,RuntimeEvent::ToolFinished{id,output} if id=="call1" && output.contains(expected))));
            assert_eq!(output.iter().filter(|e| e.is_terminal()).count(), 1);
            assert_eq!(
                output.last(),
                Some(&RuntimeEvent::Completed {
                    text: "verified".into()
                })
            );
            assert_eq!(count.load(Ordering::SeqCst), 21);
            assert_eq!(
                output
                    .iter()
                    .filter(|e| matches!(e, RuntimeEvent::ToolFinished { .. }))
                    .count(),
                20
            );
        }
    }
}
#[test]
fn chat_tool_arguments_accumulate_across_chunks() {
    let mut turn = Turn::default();
    let mut discard = |_: super::turn::Delta| {};
    ProtocolFactory::create(Api::Chat).consume(&mut turn, json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"shell","arguments":"{\"path\":"}}]}}]}), &mut discard).unwrap();
    ProtocolFactory::create(Api::Chat).consume(&mut turn, json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"README.md\"}"}}]},"finish_reason":"tool_calls"}]}), &mut discard).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(turn.arguments(0)).unwrap()["path"],
        "README.md"
    );
    assert_eq!(turn.calls()[&0]["id"], "c1");
}
