use super::{
    client::ModelClient,
    config::{HarnessConfig, ModelApi},
};
use crate::{RuntimeEvent, context::ContextBudget, tools::ToolRegistry};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[tokio::test]
async fn intelligent_compression_keeps_recent_and_preserves_original_on_failure() {
    for valid in [true, false] {
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        let app=axum::Router::new().fallback(axum::routing::post(move |axum::Json(body):axum::Json<Value>|{let counter=counter.clone();async move {
    counter.fetch_add(1,Ordering::SeqCst);
    assert!(body["messages"][1]["content"].as_str().unwrap().contains("COMPACTION TASK"));
    assert!(body["tools"].as_array().unwrap().is_empty());
    let summary=if valid {json!({"goals":["retain important objective"],"constraints":["preserve logs"],"decisions":[],"completed":[],"pending":["verify changes"],"risks":[],"references":[]}).to_string()}else{"not JSON".into()};
    ([("content-type","text/event-stream")],format!("data: {}\n\n",json!({"choices":[{"delta":{"content":summary},"finish_reason":"stop"}]})))
  }}));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = ModelClient::new(HarnessConfig {
            environment: Default::default(),
            system_prompt: crate::config::default_crabot_system_prompt().into(),
            context: ContextBudget::from_lookup(|k| match k {
                "CONTEXT_MAX_TOKENS" => Some("8192".into()),
                "CONTEXT_STRATEGY" => Some("intelligent".into()),
                _ => None,
            })
            .unwrap(),
            api: ModelApi::Chat,
            base: format!("http://{address}"),
            key: "fixture".into(),
            model: "fixture".into(),
            max_tokens: 1024,
            deepseek_effort: None,
            root: ".".into(),
        })
        .unwrap();
        let records = json!([{"role":"user","content":"important objective"},{"role":"assistant","content":"old details ".repeat(2000)},{"role":"user","content":"RECENT ORIGINAL"},{"role":"assistant","content":"RECENT ANSWER"}]);
        let mut history = vec![
            json!({"role":"user","content":format!("PROTECTED RULES\nPrevious topic records:\n{records}\ntrailing important instruction\nLatest user request:\nCURRENT EXACT REQUEST")}),
        ];
        let original = history.clone();
        let mut events = vec![];
        let result = client
            .prepare_context(&mut history, &ToolRegistry::new(), &mut |e| events.push(e))
            .await;
        server.abort();
        assert!(count.load(Ordering::SeqCst) > 0);
        assert!(
            events
                .iter()
                .all(|e| matches!(e, RuntimeEvent::ContextCheckpoint { .. }))
        );
        if valid {
            result.unwrap();
            let text = history[0]["content"].as_str().unwrap();
            assert!(text.contains("retain important objective"));
            assert!(text.contains("RECENT ORIGINAL"));
            assert!(text.contains("RECENT ANSWER"));
            assert!(text.starts_with("PROTECTED RULES"));
            assert!(text.ends_with("CURRENT EXACT REQUEST"));
        } else {
            assert!(result.is_err());
            assert_eq!(history, original);
        }
    }
}
