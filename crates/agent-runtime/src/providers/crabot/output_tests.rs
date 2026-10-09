use super::{config::HarnessConfig, protocol::ProtocolFactory};
use crate::tools::ToolRegistry;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn config(api: &str, limit: Option<&str>, base: &str) -> HarnessConfig {
    HarnessConfig::from_lookup(|key| match key {
        "MODEL_PROVIDER" => Some("compatible".into()),
        "MODEL_API" => Some(api.into()),
        "MODEL_BASE_URL" => Some(base.into()),
        "MODEL_NAME" => Some("fixture".into()),
        "MODEL_API_KEY" => Some("fixture-key".into()),
        "AGENT_WORKDIR" => Some(env!("CARGO_MANIFEST_DIR").into()),
        "HARNESS_MAX_TOKENS" => limit.map(str::to_owned),
        _ => None,
    })
    .unwrap()
}

fn body(request: reqwest::RequestBuilder) -> Value {
    let request = request.build().unwrap();
    serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap()
}

#[tokio::test]
async fn automatic_output_preserves_long_streamed_answers() {
    let answer = "long answer ".repeat(5000);
    let response_text = answer.clone();
    let app = axum::Router::new().route(
        "/chat/completions",
        axum::routing::post(move |axum::Json(request): axum::Json<Value>| {
            let answer = response_text.clone();
            async move {
                assert!(request.get("max_tokens").is_none());
                let frame =
                    json!({"choices":[{"delta":{"content":answer},"finish_reason":"stop"}]});
                (
                    [("content-type", "text/event-stream")],
                    format!("data: {frame}\n\n"),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = super::client::ModelClient::new(config("chat", None, &base)).unwrap();
    let mut streamed = String::new();
    let turn = client
        .next_turn(
            &[json!({"role":"user","content":"long answer"})],
            &ToolRegistry::new(),
            &mut |event| {
                if let crate::RuntimeEvent::TextDelta { text } = event {
                    streamed.push_str(&text);
                }
            },
        )
        .await
        .unwrap();
    assert_eq!(streamed, answer);
    assert_eq!(turn.text(), answer);
    server.abort();
}

#[tokio::test]
async fn optional_protocols_omit_limits_and_respect_explicit_values() {
    let http = reqwest::Client::new();
    let tools = ToolRegistry::new();
    for api in ["chat", "responses"] {
        for limit in [None, Some(""), Some("  "), Some("16384")] {
            let cfg = config(api, limit, "http://127.0.0.1:1");
            let protocol = ProtocolFactory::create(cfg.api);
            // Preparing optional protocols must not perform model discovery.
            let request = body(
                protocol
                    .prepare_request(&http, &cfg, &[], &tools)
                    .await
                    .unwrap(),
            );
            let field = if api == "chat" {
                "max_tokens"
            } else {
                "max_output_tokens"
            };
            if limit == Some("16384") {
                assert_eq!(request[field], 16384);
                assert_eq!(cfg.input_limit().unwrap(), 65536 - 16384);
            } else {
                assert!(request.get("max_tokens").is_none());
                assert!(request.get("max_output_tokens").is_none());
                assert!(cfg.max_tokens.is_none());
                assert_eq!(cfg.input_limit().unwrap(), 65536 * 3 / 4);
            }
        }
    }
}

#[tokio::test]
async fn anthropic_discovers_and_caches_model_limit_without_capping_it_at_4096() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let app = axum::Router::new().route(
        "/v1/models/fixture",
        axum::routing::get(move |headers: axum::http::HeaderMap| {
            count.fetch_add(1, Ordering::SeqCst);
            async move {
                assert_eq!(headers["x-api-key"], "fixture-key");
                assert_eq!(headers["anthropic-version"], "2023-06-01");
                axum::Json(json!({"max_tokens": 64000}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let cfg = config("anthropic", None, &base);
    let protocol = ProtocolFactory::create(cfg.api);
    let http = reqwest::Client::new();
    let tools = ToolRegistry::new();
    for _ in 0..2 {
        let request = body(
            protocol
                .prepare_request(&http, &cfg, &[], &tools)
                .await
                .unwrap(),
        );
        assert_eq!(request["max_tokens"], 64000);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.abort();
    let explicit = config("anthropic", Some("16384"), "http://127.0.0.1:1");
    let request = body(
        protocol
            .prepare_request(&http, &explicit, &[], &tools)
            .await
            .unwrap(),
    );
    assert_eq!(request["max_tokens"], 16384);
}

#[tokio::test]
async fn anthropic_missing_metadata_does_not_silently_restore_a_fixed_limit() {
    for value in [
        json!({}),
        json!({"max_tokens": null}),
        json!({"max_tokens": 0}),
    ] {
        let app = axum::Router::new().fallback(axum::routing::get(move || {
            let value = value.clone();
            async move { axum::Json(value) }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let cfg = config("anthropic", None, &base);
        let error = ProtocolFactory::create(cfg.api)
            .prepare_request(&reqwest::Client::new(), &cfg, &[], &ToolRegistry::new())
            .await
            .unwrap_err();
        assert!(error.contains("HARNESS_MAX_TOKENS"));
        server.abort();
    }
}
