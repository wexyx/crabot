use crate::*;
use agent_protocol::SseDecoder;
use std::collections::{HashSet, VecDeque};
use tokio::sync::Semaphore;

#[derive(Clone, Serialize, Deserialize)]
pub struct Link {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    ak: String,
    #[serde(default)]
    sk: String,
    enrollment_token: Option<String>,
    pub local_project_id: Uuid,
    #[serde(default)]
    pub local_agents: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}
fn default_timeout() -> u64 {
    900
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn routes_within_project_and_relays_token_limits() {
        let reports = Arc::new(Mutex::new(Vec::<Value>::new()));
        let received = reports.clone();
        let app = Router::new().route(
            "/v1/client/events",
            post(move |headers: HeaderMap, Json(value): Json<Value>| {
                let received = received.clone();
                async move {
                    assert_eq!(headers["x-agent-ak"], "fixture-ak");
                    received.lock().await.push(value);
                    StatusCode::ACCEPTED
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        // Routing test only: a closed lazy pool skips persistence without a real database.
        let store = storage::Store::memory();
        let state = AppState {
            policy_store: policy_store::Store::memory(),
            control_pending: Default::default(),
            sessions: Default::default(),
            clients: Default::default(),
            store,
            node_id: "node-b".into(),
            links: Arc::new(vec![]),
            link_status: Default::default(),
        };
        let project = Uuid::new_v4();
        let session = Uuid::new_v4();
        let (events, mut rx) = broadcast::channel(32);
        state.sessions.lock().await.insert(
            session,
            Session {
                project_id: project,
                client_id: None,
                events: events.clone(),
                requests: HashMap::new(),
            },
        );
        let (wrong_tx, mut wrong_rx) = mpsc::channel(8);
        state.clients.lock().await.insert(
            (Uuid::new_v4(), "second".into()),
            ClientConnection {
                sender: wrong_tx,
                role: "fixture".into(),
                node_id: None,
            },
        );
        let mut workers = vec![];
        for name in ["first", "second"] {
            let (tx, mut commands) = mpsc::channel::<WireEvent>(8);
            state.clients.lock().await.insert(
                (project, name.into()),
                ClientConnection {
                    sender: tx,
                    role: "fixture".into(),
                    node_id: None,
                },
            );
            let events = events.clone();
            let fixture_state = state.clone();
            workers.push(tokio::spawn(async move {
                let command = commands.recv().await.unwrap();
                assert_eq!(command.data["route"]["visited_nodes"], json!(["node-a", "node-b"]));
                let id = &command.data["message_id"];
                if name == "first" {
                    // Simulate the terminal cleanup performed by accept_client_event.
                    fixture_state.sessions.lock().await.get_mut(&session).unwrap().requests.clear();
                    events.send(event(session, "agent.error", json!({"message_id":id,"content":"quota exhausted","error_code":"token_insufficient"}))).unwrap();
                } else {
                    for kind in ["agent.delta", "agent.message", "agent.done"] {
                        events.send(event(session, kind, json!({"message_id":id,"content":"verified"}))).unwrap();
                    }
                }
            }));
        }
        let link = Link {
            name: "upstream".into(),
            url: format!("http://{address}"),
            client_id: "node-b-agent".into(),
            ak: "fixture-ak".into(),
            sk: "fixture-sk".into(),
            enrollment_token: None,
            local_project_id: project,
            local_agents: vec!["first".into(), "second".into()],
            timeout_secs: 10,
        };
        let mut route = RouteContext::default();
        route.enter("node-a").unwrap();
        let original = event(
            Uuid::new_v4(),
            "command",
            json!({"message_id":Uuid::new_v4(),"content":"test"}),
        );
        let answer = tokio::time::timeout(
            // Generous on purpose: this exercises a real HTTP roundtrip plus fixture
            // sleeps, and under a loaded suite (embedded-DB tests run alongside) a
            // tight budget flakes without signalling a product regression.
            Duration::from_secs(30),
            run_local(
                &state,
                &link,
                &reqwest::Client::new(),
                &original,
                session,
                &mut rx,
                route,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(answer, "verified");
        assert!(wrong_rx.try_recv().is_err());
        let reports = reports.lock().await;
        assert_eq!(reports.len(), 2);
        assert!(reports[0]["content"].as_str().unwrap().contains("token"));
        assert_eq!(reports[1]["session_id"], original.session_id.to_string());
        assert_eq!(reports[1]["message_id"], original.data["message_id"]);
        for worker in workers {
            worker.await.unwrap();
        }
        server.abort();
    }
}

pub fn configuration() -> Vec<Link> {
    let json = std::env::var("NODE_LINKS_JSON").unwrap_or_else(|_| "[]".into());
    let links: Vec<Link> =
        serde_json::from_str(&json).expect("NODE_LINKS_JSON must be an array of node links");
    let mut names = HashSet::new();
    for link in &links {
        let url = Url::parse(&link.url).expect("node link URL is invalid");
        assert!(
            matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
            "node link must use HTTP(S)"
        );
        assert!(
            names.insert(link.name.clone()) && !link.name.is_empty(),
            "node link names must be unique"
        );
        assert!(
            (!link.ak.is_empty() && !link.sk.is_empty() && !link.client_id.is_empty())
                || link.enrollment_token.is_some(),
            "node link needs an enrollment token or registered AK/SK and Client ID"
        );
        assert!(
            !link.local_agents.is_empty() && link.local_agents.iter().all(|id| !id.is_empty()),
            "explicit local_agents allowlist is required"
        );
        assert!(
            (1..=3600).contains(&link.timeout_secs),
            "link timeout must be 1..3600 seconds"
        );
    }
    links
}

pub async fn identity(store: &storage::Store) -> String {
    store
        .transaction(|data| {
            if let Some(value) = data.get("node", "identity") {
                return Ok(storage::field(value, "id"));
            }
            let id = Uuid::new_v4().to_string();
            data.set("node", "identity", json!({"id":id}));
            Ok(id)
        })
        .await
        .expect("persist node identity")
}

pub fn start(state: AppState) {
    for link in state.links.iter().cloned() {
        let state = state.clone();
        tokio::spawn(async move {
            super::link_lifecycle::supervise(state, link).await;
        });
    }
}

pub(crate) async fn restore_mounts(state: &AppState) -> Result<(), String> {
    for row in state.store.list("peer_mounts").await {
        let link: Link = serde_json::from_value(row).map_err(|e| e.to_string())?;
        if state.links.iter().any(|l| l.name == link.name) {
            continue;
        }
        let state = state.clone();
        tokio::spawn(async move {
            super::link_lifecycle::supervise(state, link).await;
        });
    }
    Ok(())
}

pub(crate) async fn mount(
    state: &AppState,
    project: Uuid,
    mut input: Value,
) -> Result<Value, String> {
    input["local_project_id"] = json!(project);
    if input["name"].as_str().is_none_or(str::is_empty) {
        input["name"] = json!(format!("peer-{}", Uuid::new_v4().simple()));
    }
    let link: Link = serde_json::from_value(input).map_err(|e| e.to_string())?;
    let url = Url::parse(&link.url).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("invalid peer URL".into());
    }
    if link.name.is_empty()
        || link.name.len() > 64
        || link.local_agents.len() > 8
        || !(1..=3600).contains(&link.timeout_secs)
    {
        return Err("invalid mount definition".into());
    }
    if state.links.iter().any(|l| l.name == link.name) {
        return Err("link name already configured".into());
    }
    for id in &link.local_agents {
        if state
            .store
            .get("local_agents", &format!("{project}:{id}"))
            .await
            .is_none()
        {
            return Err("allowlist must contain existing local business agents".into());
        }
    }
    state
        .store
        .insert("peer_mounts", &link.name, json!(link))
        .await?;
    let result = json!({"name":link.name,"status":"connecting","local_agents":link.local_agents});
    let state = state.clone();
    tokio::spawn(async move {
        super::link_lifecycle::supervise(state, link).await;
    });
    Ok(result)
}

pub(super) async fn run_link(state: AppState, mut link: Link) {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("HTTP client");
    let permits = Arc::new(Semaphore::new(4));
    let mut seen = VecDeque::new();
    loop {
        if link.ak.is_empty() {
            if let Err(error) = enroll(&state, &http, &mut link).await {
                state
                    .link_status
                    .lock()
                    .await
                    .insert(link.name.clone(), error);
                return;
            }
        }
        state
            .link_status
            .lock()
            .await
            .insert(link.name.clone(), "connecting".into());
        let result = http
            .get(format!(
                "{}/v1/client/connect",
                link.url.trim_end_matches('/')
            ))
            .header("x-agent-ak", &link.ak)
            .header("x-agent-sk", &link.sk)
            .header("x-node-id", &state.node_id)
            .send()
            .await;
        if let Ok(response) = result {
            if response.status().is_success() {
                let mut bytes = response.bytes_stream();
                let mut decoder = SseDecoder::default();
                let mut ready = false;
                let mut parent_node = String::new();
                'connection: while let Some(chunk) = bytes.next().await {
                    let Ok(chunk) = chunk else {
                        break;
                    };
                    let Ok(events) = decoder.push(&chunk) else {
                        break;
                    };
                    for command in events {
                        if command.kind == "ready" {
                            if command.data["client_id"].as_str() != Some(&link.client_id)
                                || command.data["node_id"].as_str() == Some(&state.node_id)
                            {
                                break 'connection;
                            }
                            ready = true;
                            parent_node =
                                command.data["node_id"].as_str().unwrap_or_default().into();
                            state
                                .link_status
                                .lock()
                                .await
                                .insert(link.name.clone(), "connected".into());
                            continue;
                        }
                        if ready && command.kind == "control.request" {
                            if let Ok(request) = serde_json::from_value::<control::Request>(
                                command.data["request"].clone(),
                            ) {
                                let (state, link, http, parent) = (
                                    state.clone(),
                                    link.clone(),
                                    http.clone(),
                                    parent_node.clone(),
                                );
                                let permit = permits.clone().try_acquire_owned();
                                tokio::spawn(async move {
                                    let id = request.id;
                                    let result = match permit {
                                        Ok(_permit) => {
                                            control::receive(
                                                state,
                                                link.local_project_id,
                                                parent,
                                                request,
                                            )
                                            .await
                                        }
                                        Err(_) => Err("Crabot busy".into()),
                                    };
                                    let body = match result {
                                        Ok(result) => json!({"id":id,"result":result}),
                                        Err(error) => json!({"id":id,"error":error}),
                                    };
                                    let _ = http
                                        .post(format!(
                                            "{}/v1/client/control-results",
                                            link.url.trim_end_matches('/')
                                        ))
                                        .header("x-agent-ak", &link.ak)
                                        .header("x-agent-sk", &link.sk)
                                        .timeout(Duration::from_secs(20))
                                        .json(&body)
                                        .send()
                                        .await;
                                });
                            }
                            continue;
                        }
                        if !ready || command.kind != "command" {
                            continue;
                        }
                        if seen.contains(&command.id) {
                            continue;
                        }
                        let Ok(permit) = permits.clone().try_acquire_owned() else {
                            let _ = report(
                                &http,
                                &link,
                                &command,
                                "failed",
                                "node busy; maximum 4 in-flight tasks",
                                None,
                            )
                            .await;
                            continue;
                        };
                        seen.push_back(command.id);
                        if seen.len() > 4096 {
                            seen.pop_front();
                        }
                        let (state, link, http) = (state.clone(), link.clone(), http.clone());
                        tokio::spawn(async move {
                            let _permit = permit;
                            handle_task(state, link, http, command).await;
                        });
                    }
                }
            }
        }
        state
            .link_status
            .lock()
            .await
            .insert(link.name.clone(), "failed".into());
        return;
    }
}

async fn enroll(state: &AppState, http: &reqwest::Client, link: &mut Link) -> Result<(), String> {
    if let Some(row) = state.store.get("node_link_credentials", &link.name).await {
        if row["remote_url"] != json!(link.url) {
            return Err("link URL changed; use a new link name and enrollment token".into());
        }
        link.client_id = storage::field(&row, "client_id");
        link.ak = storage::field(&row, "ak");
        link.sk = storage::field(&row, "sk");
        return Ok(());
    }
    let auto_key = format!("registration:{}", link.name);
    let registration = state
        .store
        .transaction(|d| {
            if let Some(row) = d.get("node_link_credentials", &auto_key) {
                return Ok(row.clone());
            }
            let row =
                json!({"secret":format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple())});
            d.insert("node_link_credentials", &auto_key, row.clone())?;
            Ok(row)
        })
        .await?;
    let registration_body = if link
        .enrollment_token
        .as_deref()
        .is_some_and(|s| !s.is_empty())
    {
        json!({"enrollment_token":link.enrollment_token})
    } else {
        json!({"node_id":state.node_id,"registration_secret":registration["secret"],"name":std::env::var("AGENT_NAME").unwrap_or_else(|_|"Crabot".into())})
    };
    let response = http
        .post(format!(
            "{}/v1/client/register",
            link.url.trim_end_matches('/')
        ))
        .timeout(Duration::from_secs(20))
        .json(&registration_body)
        .send()
        .await
        .map_err(|_| "node registration connection failed")?;
    if !response.status().is_success() {
        return Err(format!("node registration rejected: {}", response.status()));
    }
    #[derive(Deserialize)]
    struct Keys {
        client_id: String,
        ak: String,
        sk: String,
    }
    let keys: Keys = response
        .json()
        .await
        .map_err(|_| "invalid node registration response")?;
    state
        .store
        .insert(
            "node_link_credentials",
            &link.name,
            json!({"remote_url":link.url,"client_id":keys.client_id,"ak":keys.ak,"sk":keys.sk}),
        )
        .await
        .map_err(|_| "cannot save node credentials; registration token already consumed")?;
    link.client_id = keys.client_id;
    link.ak = keys.ak;
    link.sk = keys.sk;
    Ok(())
}

async fn report(
    http: &reqwest::Client,
    link: &Link,
    original: &WireEvent,
    status: &str,
    content: &str,
    code: Option<&str>,
) -> Result<(), String> {
    let base = link.url.trim_end_matches('/');
    let kind = match status {
        "working" => "agent.delta",
        "completed" => "agent.message",
        "done" => "agent.done",
        _ => "agent.error",
    };
    let url = format!("{base}/v1/client/events");
    let body = json!({"session_id":original.session_id,"message_id":original.data["message_id"],"type":kind,"content":content,"error_code":code});

    let response = http
        .post(url)
        .header("x-agent-ak", &link.ak)
        .header("x-agent-sk", &link.sk)
        .timeout(Duration::from_secs(20))
        .json(&body)
        .send()
        .await
        .map_err(|_| "upstream report transport failed")?;
    if !response.status().is_success() {
        return Err(format!("upstream rejected report: {}", response.status()));
    }
    Ok(())
}

async fn handle_task(state: AppState, link: Link, http: reqwest::Client, original: WireEvent) {
    let route: RouteContext = match serde_json::from_value(original.data["route"].clone()) {
        Ok(route) => route,
        Err(_) => {
            let _ = report(
                &http,
                &link,
                &original,
                "failed",
                "node task requires route context; update the upstream node",
                None,
            )
            .await;
            return;
        }
    };
    if let Err(error) = route.clone().enter(&state.node_id) {
        let _ = report(
            &http,
            &link,
            &original,
            "failed",
            error,
            Some("routing_loop"),
        )
        .await;
        return;
    }
    if state
        .store
        .get("projects", &link.local_project_id.to_string())
        .await
        .is_none()
    {
        let _ = report(
            &http,
            &link,
            &original,
            "failed",
            "configured local project does not exist",
            None,
        )
        .await;
        return;
    }
    let session_id = Uuid::new_v4();
    let (events, mut rx) = broadcast::channel(256);
    state.sessions.lock().await.insert(
        session_id,
        Session {
            project_id: link.local_project_id,
            client_id: None,
            events,
            requests: HashMap::new(),
        },
    );
    let result = tokio::time::timeout(
        Duration::from_secs(link.timeout_secs),
        run_local(&state, &link, &http, &original, session_id, &mut rx, route),
    )
    .await;
    state.sessions.lock().await.remove(&session_id);
    let result = result.unwrap_or_else(|_| Err(("node task timed out".into(), None)));
    let report_result = match result {
        Ok(answer) => {
            let delivered = report(&http, &link, &original, "completed", &answer, None).await;
            if delivered.is_ok() && original.kind == "command" {
                report(&http, &link, &original, "done", "", None).await
            } else {
                delivered
            }
        }
        Err((error, code)) => report(&http, &link, &original, "failed", &error, code).await,
    };
    if let Err(error) = report_result {
        eprintln!("Node link {}: {error}", link.name);
    }
}

async fn run_local(
    state: &AppState,
    link: &Link,
    http: &reqwest::Client,
    original: &WireEvent,
    session_id: Uuid,
    rx: &mut broadcast::Receiver<WireEvent>,
    route: RouteContext,
) -> Result<String, (String, Option<&'static str>)> {
    let prompt = original.data["instruction"]
        .as_str()
        .or_else(|| original.data["content"].as_str())
        .unwrap_or_default();
    let mut last_limit = None;
    for agent in &link.local_agents {
        let online = state
            .clients
            .lock()
            .await
            .get(&(link.local_project_id, agent.clone()))
            .is_some_and(|c| !c.sender.is_closed());
        if !online {
            continue;
        }
        let accepted = crate::core::messages::send(
            state,
            session_id,
            SendMessage {
                content: prompt.into(),
                client_id: Some(agent.clone()),
                route: Some(route.clone()),
            },
        )
        .await
        .map_err(|(_, error)| (error.0.to_string(), None))?;
        let message_id = accepted["message_id"].clone();
        let mut answer = String::new();
        loop {
            let event = rx
                .recv()
                .await
                .map_err(|_| ("local event stream closed or lagged".into(), None))?;
            if event.data["message_id"] != message_id {
                continue;
            }
            let text = event.data["content"].as_str().unwrap_or_default();
            match event.kind.as_str() {
                "agent.delta" | "agent.context" | "agent.tool.started" | "agent.tool.finished" => {
                    report(http, link, original, "working", text, None)
                        .await
                        .map_err(|e| (e, None))?
                }
                "agent.message" => answer = text.into(),
                "agent.done" => return Ok(answer),
                "agent.error" => {
                    if event.data["error_code"] == "token_insufficient"
                        || text.starts_with("TOKEN_INSUFFICIENT:")
                    {
                        last_limit = Some(text.to_owned());
                        report(
                            http,
                            link,
                            original,
                            "working",
                            &format!(
                                "\n[{} / {} token 不足，尝试下一个 Agent]\n",
                                state.node_id, agent
                            ),
                            None,
                        )
                        .await
                        .map_err(|e| (e, None))?;
                        break;
                    }
                    return Err((text.to_owned(), None));
                }
                _ => {}
            }
        }
    }
    match last_limit {
        Some(error) => Err((error, Some("token_insufficient"))),
        None => Err((
            "no online agents in this link's local allowlist".into(),
            None,
        )),
    }
}
