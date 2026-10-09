use super::*;
use crate::{conversation, core::Core, crabot_tests};
use agent_runtime::{
    config::{HarnessConfig, ModelApi, RuntimeConfig},
    skills::{ExecutionPolicy, SkillCatalog},
    tools::{ToolContext, ToolFactory, ToolSession},
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;
async fn fixture() -> (Arc<Manager>, Uuid) {
    let manager = Manager::new(Core::new(crabot_tests::state("manager").await));
    let project = manager.core().bootstrap().await.unwrap();
    (manager, project)
}
#[tokio::test]
async fn capability_packages_are_disjoint_and_skills_are_scoped() {
    let (m, p) = fixture().await;
    let manager = m.registry(p).await.unwrap();
    let names = manager
        .definitions()
        .iter()
        .map(|d| d.name().to_string())
        .collect::<Vec<_>>();
    assert!(names.contains(&"group_create".into()) && names.contains(&"find".into()));
    assert!(names.contains(&"compact".into()));
    assert!(!names.contains(&"python_run".into()) && !names.contains(&"read".into()));
    let business = ToolFactory::create(
        ToolContext::new(
            None,
            SkillCatalog::default(),
            ExecutionPolicy::new("offline".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    for registry in [&manager, &business] {
        for strategy in ["summary", "recent"] {
            let result = registry
                .execute(
                    "compact",
                    &json!({"strategy":strategy}),
                    &mut ToolSession::default(),
                )
                .await
                .unwrap();
            assert_eq!(result["status"], "scheduled");
        }
    }
    assert!(
        !business
            .definitions()
            .iter()
            .any(|d| d.name() == "group_create")
    );
    assert!(
        business
            .execute("agent_start", &json!({}), &mut ToolSession::default())
            .await
            .is_err()
    );
    let other = Uuid::new_v4();
    m.core()
        .state()
        .store
        .insert("projects", &other.to_string(), json!({"id":other}))
        .await
        .unwrap();
    skills::save(m.core(),p,json!({"expected_version":0,"definition":{"id":"custom","description":"custom management guide","enabled":true,"allow_python":false,"files":{"SKILL.md":"Manage carefully"}}})).await.unwrap();
    assert_eq!(
        skills::catalog(m.core(), p)
            .await
            .unwrap()
            .definitions()
            .len(),
        2
    );
    assert_eq!(
        skills::catalog(m.core(), other)
            .await
            .unwrap()
            .definitions()
            .len(),
        1
    );
    assert!(
        crate::skills::snapshot(m.core().state(), p)
            .await
            .unwrap()
            .definitions()
            .iter()
            .all(|skill| skill.id() != "custom")
    );
}
#[tokio::test]
async fn stop_requires_a_human_scoped_single_use_confirmation() {
    let (m, p) = fixture().await;
    let tools = m.registry(p).await.unwrap();
    let mut session = ToolSession::default();
    tools
        .execute(
            "agent_start",
            &json!({"client_id":"worker","role":"dev","provider":"mock"}),
            &mut session,
        )
        .await
        .unwrap();
    let pending = tools
        .execute("agent_stop", &json!({"client_id":"worker"}), &mut session)
        .await
        .unwrap();
    let id = serde_json::from_value(pending["id"].clone()).unwrap();
    assert_eq!(
        m.core().agents(p).await.unwrap().as_array().unwrap().len(),
        1
    );
    assert!(m.core().decide(Uuid::new_v4(), id, true).await.is_err());
    m.core().decide(p, id, true).await.unwrap();
    assert!(m.core().decide(p, id, true).await.is_err());
    assert!(
        m.core()
            .agents(p)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn web_lifecycle_does_not_own_agent_lifecycle() {
    let (m, p) = fixture().await;
    m.start("mock").await.unwrap();
    m.core()
        .agent_start(
            p,
            json!({"client_id":"worker","role":"dev","provider":"mock"}),
        )
        .await
        .unwrap();
    let url = m.web().start(m.clone(), "127.0.0.1:0").await.unwrap();
    assert!(
        reqwest::get(format!("{url}/healthz"))
            .await
            .unwrap()
            .status()
            .is_success()
    );
    m.web().stop().await;
    assert_eq!(
        m.core().agents(p).await.unwrap().as_array().unwrap().len(),
        2
    );
    let id = serde_json::from_value(m.create_session(p).await.unwrap()["id"].clone()).unwrap();
    m.message(p, id, "hello".into()).await.unwrap();
    wait(&m, p, id).await;
    assert_eq!(m.history(p, id).await.unwrap()["status"], "completed");
}

#[tokio::test]
async fn remote_web_protects_management_assets_attachments_and_streams() {
    let (manager, _) = fixture().await;
    let token = "remote-web-integration-test-token-123456";
    let access = crate::http::web_access::WebAccess::new(Some(token), "").unwrap();
    let app =
        crate::http::routes::router_with_manager(manager.core().state().clone(), manager, access);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let client = reqwest::Client::new();
    for path in [
        "/",
        "/assets/test.js",
        "/v1/agent",
        "/v1/repl",
        "/v1/attachments/id/content",
        "/v1/sessions/00000000-0000-0000-0000-000000000000/events",
    ] {
        let response = client
            .get(format!("http://{address}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED,
            "{path}"
        );
        assert!(
            response
                .headers()
                .get("www-authenticate")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Basic")
        );
    }
    let url = format!("http://{address}/v1/agent");
    assert!(
        client
            .get(&url)
            .basic_auth("crabot", Some(token))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    assert!(
        client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    assert_eq!(
        client
            .get(&url)
            .basic_auth("crabot", Some("wrong"))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .get(&url)
            .basic_auth("crabot", Some(token))
            .header("origin", "https://evil.example")
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    assert!(
        client
            .get(format!("http://{address}/healthz"))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    task.abort();
    let _ = task.await;
}
async fn wait(m: &Arc<Manager>, p: Uuid, id: Uuid) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if m.history(p, id).await.unwrap()["status"] != "running" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn natural_language_harness_calls_registered_tools_and_persists_history() {
    use axum::{Json, Router, http::header, routing::post};
    let step = Arc::new(AtomicUsize::new(0));
    let received = step.clone();
    let app=Router::new().route("/v1/chat/completions",post(move|Json(body):Json<Value>|{
        let received=received.clone();
        async move{
            assert!(!body["tools"].as_array().unwrap().iter().any(|t|t["function"]["name"]=="python_run"));
            let n=received.fetch_add(1,Ordering::SeqCst);
            let chunk=match n{
                0=>call("find",json!({"target":"skill","id":"management-guide"})),
                1=>call("agent_start",json!({"client_id":"worker","role":"developer","provider":"mock"})),
                2=>call("group_create",json!({"name":"Dev","policy":{"mode":"relay","members":[{"path":["worker"],"role":"developer"}],"rounds":1,"instructions":"","leader":null}})),
                _=>json!({"choices":[{"delta":{"content":"Group created"},"finish_reason":"stop"}]}),
            };
            ([(header::CONTENT_TYPE,"text/event-stream")],format!("data: {chunk}\n\ndata: [DONE]\n\n"))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let (m, p) = fixture().await;
    m.configure(RuntimeConfig::Crabot(HarnessConfig {
        environment: Default::default(),
        context: Default::default(),
        system_prompt: agent_runtime::config::default_crabot_system_prompt().into(),
        api: ModelApi::Chat,
        base,
        key: "fixture".into(),
        model: "fixture".into(),
        max_tokens: Some(1024),
        deepseek_effort: None,
        root: ".".into(),
    }))
    .await
    .unwrap();
    let id = serde_json::from_value(m.create_session(p).await.unwrap()["id"].clone()).unwrap();
    m.message(p, id, "建立一个研发接力群".into()).await.unwrap();
    // Running conversations now accept steering; its boundary/persistence behavior
    // is covered separately by the gated model integration fixture.
    wait(&m, p, id).await;
    let row = m.history(p, id).await.unwrap();
    assert_eq!(row["status"], "completed", "{row}");
    let groups = m.core().control(p, "group.list", json!({})).await.unwrap();
    assert_eq!(groups.as_array().unwrap().len(), 1);
    let result = m
        .core()
        .group_chat(p, json!({"group_id":groups[0]["key"],"content":"hello"}))
        .await
        .unwrap();
    let business = serde_json::from_value(result["id"].clone()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if m.core()
                .history(p, business)
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["type"] == "agent.done")
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let restored = Manager::new(m.core().clone());
    assert_eq!(restored.history(p, id).await.unwrap(), row);
    assert!(restored.history(Uuid::new_v4(), id).await.is_err());
    server.abort();
}
fn call(name: &str, args: Value) -> Value {
    json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":format!("call-{name}"),"type":"function","function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":"tool_calls"}]})
}
#[tokio::test]
async fn restart_marks_admin_work_interrupted_without_reexecution() {
    let (m, p) = fixture().await;
    let row = m.create_session(p).await.unwrap();
    let id: Uuid = serde_json::from_value(row["id"].clone()).unwrap();
    m.core()
        .state()
        .store
        .transaction(|d| {
            let mut row = row;
            row["status"] = json!("running");
            d.set("management_sessions", &id.to_string(), row);
            Ok(())
        })
        .await
        .unwrap();
    conversation::recover(m.core().state()).await.unwrap();
    let row = m.history(p, id).await.unwrap();
    assert_eq!(row["status"], "interrupted");
    assert_eq!(row["events"][0]["type"], "failed");
}
