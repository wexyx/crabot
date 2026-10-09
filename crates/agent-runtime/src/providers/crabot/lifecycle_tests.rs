use crate::{
    RuntimeFactory,
    config::{HarnessConfig, ModelApi, RuntimeConfig},
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[tokio::test]
async fn native_tool_owner_does_not_reenter_json_bridge_or_wait_for_http_eof() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let answer = json!({"crabot_tool":{"name":"shell","command":"cat Cargo.toml"}}).to_string();
    let expected = answer.clone();
    let app = axum::Router::new().fallback(axum::routing::post(
        move |axum::Json(body): axum::Json<Value>| {
            let count = count.clone();
            let answer = answer.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                assert!(
                    !body["messages"]
                        .to_string()
                        .contains("PROJECT SKILL SERVICE")
                );
                assert!(!body["tools"].as_array().unwrap().is_empty());
                let event =
                    json!({"choices":[{"delta":{"content":answer},"finish_reason":"stop"}]});
                let stream = futures_util::stream::once(async move {
                    Ok::<_, std::convert::Infallible>(format!("data: {event}\n\n"))
                })
                .chain(futures_util::stream::pending());
                (
                    [("content-type", "text/event-stream")],
                    axum::body::Body::from_stream(stream),
                )
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let runtime = RuntimeFactory::from_config(RuntimeConfig::Crabot(HarnessConfig {
        environment: Default::default(),
        context: Default::default(),
        system_prompt: crate::config::default_crabot_system_prompt().into(),
        api: ModelApi::Chat,
        base: format!("http://{address}"),
        key: "fixture".into(),
        model: "fixture".into(),
        max_tokens: 100,
        deepseek_effort: None,
        root: env!("CARGO_MANIFEST_DIR").into(),
    }))
    .unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        runtime.run("Return a JSON example", &mut |_| {}),
    )
    .await;
    server.abort();
    assert_eq!(
        result
            .expect("protocol completion must not wait for socket EOF")
            .unwrap(),
        expected
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "Crabot must own exactly one tool loop"
    );
}
