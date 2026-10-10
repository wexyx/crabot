use crate::{core::Core, *};
use policies::{Member, Mode, Policy};

#[tokio::test]
async fn durable_context_is_scoped_ordered_and_keeps_interrupted_progress() {
    let s = state("context").await;
    let p = Uuid::new_v4();
    let topic = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    for (project, session, kind, id, text) in [
        (p, topic, "message.created", first, "original requirement"),
        (p, topic, "agent.tool.finished", first, "saved checkpoint"),
        (p, topic, "agent.delta", first, "partial work"),
        (p, topic, "message.created", second, "amend requirement"),
        (
            Uuid::new_v4(),
            topic,
            "agent.message",
            first,
            "OTHER PROJECT SECRET",
        ),
        (
            p,
            Uuid::new_v4(),
            "agent.message",
            first,
            "OTHER TOPIC SECRET",
        ),
    ] {
        assert!(
            persist_event(
                &s,
                project,
                &format!("session:{session}"),
                &event(session, kind, json!({"message_id":id,"content":text}))
            )
            .await
        );
    }
    let text = conversation::context(&s, p, topic, Some(second), "local")
        .await
        .unwrap();
    assert!(text.contains("original requirement"));
    assert!(text.contains("saved checkpoint"));
    assert!(text.contains("partial work"));
    assert!(!text.contains("amend requirement"));
    assert!(!text.contains("SECRET"));
}

#[tokio::test]
async fn interrupt_fences_late_output_and_restart_never_reexecutes() {
    let s = state("interrupt").await;
    let p = Uuid::new_v4();
    let topic = Uuid::new_v4();
    let id = Uuid::new_v4();
    let (events, _) = broadcast::channel(16);
    s.sessions.lock().await.insert(
        topic,
        Session {
            project_id: p,
            client_id: Some("worker".into()),
            events,
            requests: HashMap::from([(id, "worker".into())]),
        },
    );
    s.store
        .insert(
            "runs",
            &id.to_string(),
            json!({"id":id,"session_id":topic,"project_id":p,"status":"running","local":true}),
        )
        .await
        .unwrap();
    let _ = conversation::interrupt_session(&s, topic).await.unwrap();
    assert!(conversation::stopped(&s, id).await);
    assert!(
        !persist_event(
            &s,
            p,
            &format!("session:{topic}"),
            &event(
                topic,
                "agent.message",
                json!({"message_id":id,"content":"late"})
            )
        )
        .await
    );
    assert!(
        accept_client_event(
            s.clone(),
            Credential {
                project_id: p,
                client_id: "worker".into(),
                role: "worker".into()
            },
            ClientEvent {
                session_id: topic,
                message_id: id,
                kind: "agent.done".into(),
                content: None,
                error: None,
                error_code: None
            }
        )
        .await
        .is_err()
    );
    let queued = Uuid::new_v4();
    s.store
        .insert(
            "runs",
            &queued.to_string(),
            json!({"id":queued,"session_id":topic,"status":"queued"}),
        )
        .await
        .unwrap();
    conversation::recover(&s).await.unwrap();
    assert!(conversation::stopped(&s, queued).await);
    assert!(s.clients.lock().await.is_empty());
}

#[tokio::test]
async fn old_management_api_is_retired_and_chat_transport_requires_admin() {
    let state = state("http-fixture").await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let client = reqwest::Client::new();
    for path in [
        "/v1/crabot/control",
        "/v1/crabot/runs",
        "/v1/spaces",
        "/v1/management/start",
    ] {
        assert_eq!(
            client
                .post(format!("{url}{path}"))
                .header("x-admin-token", "test-admin")
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        client
            .get(format!("{url}/v1/repl"))
            .header("origin", "https://evil.example")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .get(format!("{url}/v1/repl"))
            .header("x-admin-token", "test-admin")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    server.abort();
}

#[tokio::test]
async fn nested_subgroups_freeze_recursively_and_keep_context_between_rounds() {
    let a = state("a").await;
    let b = state("b").await;
    let c = state("c").await;
    let (pa, pb, pc) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let logs = Arc::new(Mutex::new(Vec::new()));
    executor(&c, pc, "worker", logs.clone()).await;
    mount(&a, pa, "b", &b, pb).await;
    mount(&b, pb, "c", &c, pc).await;
    call(
        &c,
        pc,
        &[],
        "template.update",
        json!({"expected_version":0,"policy":relay("worker")}),
    )
    .await
    .unwrap();
    let mut discussion = relay("c");
    discussion.mode = Mode::A2a;
    discussion.rounds = 2;
    call(
        &b,
        pb,
        &[],
        "template.update",
        json!({"expected_version":0,"policy":discussion}),
    )
    .await
    .unwrap();
    let group = call(
        &a,
        pa,
        &[],
        "group.create",
        json!({"name":"nested","policy":relay("b")}),
    )
    .await
    .unwrap();
    let detail = call(&a, pa, &[], "group.get", json!({"key":group["key"]}))
        .await
        .unwrap();
    let key = detail["bindings"][0]["body"]["subgroup_key"].clone();
    let snapshot = call(
        &a,
        pa,
        &["b"],
        "execution.freeze",
        json!({"key":key,"content":"research"}),
    )
    .await
    .unwrap();
    let frozen_child = &snapshot["body"]["members"]["c"];
    let mut changed = relay("worker");
    changed.instructions = "new-only".into();
    call(
        &c,
        pc,
        &[],
        "subgroup.update",
        json!({"key":frozen_child["body"]["subgroup_key"],"expected_version":1,"policy":changed}),
    )
    .await
    .unwrap();
    let result = call(
        &a,
        pa,
        &["b"],
        "task.run",
        json!({"execution_id":snapshot["key"],"invocation_id":Uuid::new_v4(),"content":"research"}),
    )
    .await
    .unwrap();
    assert!(
        result["answer"]
            .as_str()
            .unwrap()
            .contains("worker completed")
    );
    let logs = logs.lock().await;
    assert_eq!(
        logs.len(),
        1,
        "a single-member discussion needs no repeated rounds"
    );
    assert!(!logs.iter().any(|s| s.contains("new-only")));
}

pub(crate) async fn state(id: &str) -> AppState {
    let store = storage::Store::memory();
    AppState {
        policy_store: policy_store::Store::memory(),
        control_pending: Default::default(),
        sessions: Default::default(),
        clients: Default::default(),
        store,
        node_id: id.into(),
        links: Arc::new(vec![]),
        link_status: Default::default(),
    }
}
fn relay(id: &str) -> Policy {
    Policy {
        relay_strategy: Default::default(),
        mode: Mode::Relay,
        members: vec![Member {
            path: vec![id.into()],
            role: "worker".into(),
        }],
        leader: None,
        rounds: 1,
        instructions: String::new(),
    }
}
async fn call(
    s: &AppState,
    p: Uuid,
    path: &[&str],
    op: &str,
    input: Value,
) -> Result<Value, String> {
    control::call(
        s,
        p,
        path.iter().map(|x| (*x).into()).collect(),
        op.into(),
        input,
        vec![],
    )
    .await
}

async fn mount(parent: &AppState, p: Uuid, name: &str, child: &AppState, q: Uuid) {
    let (tx, mut rx) = mpsc::channel::<WireEvent>(32);
    parent.clients.lock().await.insert(
        (p, name.into()),
        ClientConnection {
            sender: tx,
            role: "Crabot".into(),
            node_id: Some(child.node_id.clone()),
        },
    );
    let (parent, child, name) = (parent.clone(), child.clone(), name.to_owned());
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let request: control::Request =
                serde_json::from_value(event.data["request"].clone()).unwrap();
            let (parent, child, name) = (parent.clone(), child.clone(), name.clone());
            tokio::spawn(async move {
                let id = request.id;
                let result = control::receive(child, q, parent.node_id.clone(), request).await;
                let reply = match result {
                    Ok(v) => control::Reply {
                        id,
                        result: Some(v),
                        error: None,
                    },
                    Err(e) => control::Reply {
                        id,
                        result: None,
                        error: Some(e),
                    },
                };
                let _ = control::finish(
                    &parent,
                    &Credential {
                        project_id: p,
                        client_id: name,
                        role: "Crabot".into(),
                    },
                    reply,
                )
                .await;
            });
        }
    });
}
async fn executor(s: &AppState, p: Uuid, id: &str, logs: Arc<Mutex<Vec<String>>>) {
    scripted_executor(s, p, id, logs, vec![]).await;
}
async fn scripted_executor(
    s: &AppState,
    p: Uuid,
    id: &str,
    logs: Arc<Mutex<Vec<String>>>,
    replies: Vec<&str>,
) {
    let mut replies = replies
        .into_iter()
        .map(str::to_owned)
        .collect::<std::collections::VecDeque<_>>();
    let (tx, mut rx) = mpsc::channel::<WireEvent>(32);
    s.clients.lock().await.insert(
        (p, id.into()),
        ClientConnection {
            sender: tx,
            role: id.into(),
            node_id: Some(s.node_id.clone()),
        },
    );
    let (s, id) = (s.clone(), id.to_owned());
    tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            let prompt = command.data["content"].as_str().unwrap();
            logs.lock().await.push(format!("{id}:{prompt}"));
            let (kind, answer) = if let Some(answer) = replies.pop_front() {
                ("agent.message", answer)
            } else if id == "limited" {
                ("agent.error", "TOKEN_INSUFFICIENT: fixture".into())
            } else if prompt.contains("RELAY NEGOTIATION ONLY") {
                (
                    "agent.message",
                    json!({"priority":if id=="worker"{95}else{20},"reason":"role match"})
                        .to_string(),
                )
            } else if id == "chatty" && prompt.contains("Return ONLY JSON") {
                // How a model actually answers when told to return only JSON.
                (
                    "agent.message",
                    "Sure, here is the split:\n```json\n{\"assignments\":[{\"member\":\"worker\",\"instruction\":\"do research\"}]}\n```\nLet me know if you want it rebalanced.".into(),
                )
            } else if id == "reluctant" && prompt.contains("previous answer was rejected") {
                // Tells itself off the first time, then answers properly.
                (
                    "agent.message",
                    r#"{"assignments":[{"member":"worker","instruction":"do research"}]}"#.into(),
                )
            } else if id == "reluctant" {
                ("agent.message", "I'd rather not split this up.".into())
            } else if id == "stubborn" && prompt.contains("Return ONLY JSON") {
                ("agent.message", "No.".into())
            } else if prompt.contains("Return ONLY JSON") {
                (
                    "agent.message",
                    r#"{"assignments":[{"member":"worker","instruction":"do research"}]}"#.into(),
                )
            } else {
                ("agent.message", format!("{id} completed"))
            };
            let cred = Credential {
                project_id: p,
                client_id: id.clone(),
                role: id.clone(),
            };
            let message_id = serde_json::from_value(command.data["message_id"].clone()).unwrap();
            let _ = accept_client_event(
                s.clone(),
                cred.clone(),
                ClientEvent {
                    session_id: command.session_id,
                    message_id,
                    kind: kind.into(),
                    content: Some(answer),
                    error: None,
                    error_code: None,
                },
            )
            .await;
            if kind != "agent.error" {
                let _ = accept_client_event(
                    s.clone(),
                    cred,
                    ClientEvent {
                        session_id: command.session_id,
                        message_id,
                        kind: "agent.done".into(),
                        content: None,
                        error: None,
                        error_code: None,
                    },
                )
                .await;
            }
        }
    });
}

#[tokio::test]
async fn copied_policies_are_shared_versioned_and_isolated_from_defaults_and_other_groups() {
    let (a, b) = (state("A").await, state("B").await);
    let (p, q) = (Uuid::new_v4(), Uuid::new_v4());
    mount(&a, p, "b", &b, q).await;
    b.policy_store
        .put(
            q,
            "template",
            "default",
            0,
            json!({"policy":relay("worker")}),
        )
        .await
        .unwrap();
    let g = Uuid::new_v4().to_string();
    let h = Uuid::new_v4().to_string();
    let bg = call(&a, p, &["b"], "subgroup.ensure", json!({"group_id":g}))
        .await
        .unwrap();
    let bh = call(&a, p, &["b"], "subgroup.ensure", json!({"group_id":h}))
        .await
        .unwrap();
    let mut edited = relay("worker");
    edited.mode = Mode::A2a;
    edited.rounds = 2;
    let v2 = call(
        &a,
        p,
        &["b"],
        "subgroup.update",
        json!({"key":bg["key"],"expected_version":1,"policy":edited}),
    )
    .await
    .unwrap();
    assert_eq!(v2["version"], 2);
    assert_eq!(
        call(
            &b,
            q,
            &[],
            "subgroup.update",
            json!({"key":bg["key"],"expected_version":1,"policy":relay("worker")})
        )
        .await
        .unwrap_err(),
        "version_conflict"
    );
    edited.instructions = "B edited this shared copy".into();
    call(
        &b,
        q,
        &[],
        "subgroup.update",
        json!({"key":bg["key"],"expected_version":2,"policy":edited}),
    )
    .await
    .unwrap();
    let shared = call(&a, p, &["b"], "subgroup.get", json!({"key":bg["key"]}))
        .await
        .unwrap();
    assert_eq!(shared["version"], 3);
    assert_eq!(
        shared["body"]["policy"]["instructions"],
        "B edited this shared copy"
    );
    call(
        &b,
        q,
        &[],
        "template.update",
        json!({"expected_version":1,"policy":edited}),
    )
    .await
    .unwrap();
    let untouched = call(&b, q, &[], "subgroup.get", json!({"key":bh["key"]}))
        .await
        .unwrap();
    assert_eq!(untouched["version"], 1);
    assert_eq!(untouched["body"]["policy"]["mode"], "relay");
    let same = call(&a, p, &["b"], "subgroup.ensure", json!({"group_id":g}))
        .await
        .unwrap();
    assert_eq!(same["version"], 3);
}

#[tokio::test]
async fn snapshots_keep_the_old_policy_and_support_multiple_phases_without_duplicate_execution() {
    let (a, b) = (state("A").await, state("B").await);
    let (p, q) = (Uuid::new_v4(), Uuid::new_v4());
    let logs = Arc::new(Mutex::new(vec![]));
    executor(&b, q, "worker", logs.clone()).await;
    mount(&a, p, "b", &b, q).await;
    b.policy_store
        .put(
            q,
            "template",
            "default",
            0,
            json!({"policy":relay("worker")}),
        )
        .await
        .unwrap();
    let bg = call(
        &a,
        p,
        &["b"],
        "subgroup.ensure",
        json!({"group_id":Uuid::new_v4()}),
    )
    .await
    .unwrap();
    let frozen = call(
        &a,
        p,
        &["b"],
        "execution.freeze",
        json!({"key":bg["key"],"content":"initial"}),
    )
    .await
    .unwrap();
    let mut changed = relay("worker");
    changed.instructions = "NEW POLICY".into();
    call(
        &b,
        q,
        &[],
        "subgroup.update",
        json!({"key":bg["key"],"expected_version":1,"policy":changed}),
    )
    .await
    .unwrap();
    let invocation = Uuid::new_v4();
    let input =
        json!({"execution_id":frozen["key"],"invocation_id":invocation,"content":"phase one"});
    let answer = call(&a, p, &["b"], "task.run", input.clone())
        .await
        .unwrap();
    assert_eq!(answer["policy_version"], 1);
    assert_eq!(
        call(&a, p, &["b"], "task.run", input).await.unwrap_err(),
        "version_conflict"
    );
    call(
        &a,
        p,
        &["b"],
        "task.run",
        json!({"execution_id":frozen["key"],"invocation_id":Uuid::new_v4(),"content":"phase two"}),
    )
    .await
    .unwrap();
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 2);
    assert!(logs[0].contains("phase one") && logs[1].contains("phase two"));
    assert!(logs.iter().all(|x| !x.contains("NEW POLICY")));
}

#[tokio::test]
async fn discovery_is_recursive_but_never_walks_up_and_replies_are_identity_bound() {
    let (a, b, c) = (state("A").await, state("B").await, state("C").await);
    let (p, q, r) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    mount(&a, p, "b", &b, q).await;
    mount(&b, q, "c", &c, r).await;
    let tree = call(&a, p, &[], "tree.get", json!({})).await.unwrap();
    assert_eq!(
        tree["children"][0]["node"]["children"][0]["node"]["id"],
        "C"
    );
    assert!(call(&b, q, &[".."], "tree.get", json!({})).await.is_err());
    assert!(call(&b, q, &["a"], "tree.get", json!({})).await.is_err());
    assert!(
        call(&a, Uuid::new_v4(), &["b"], "tree.get", json!({}))
            .await
            .is_err()
    );
    let id = Uuid::new_v4();
    let (tx, _) = tokio::sync::oneshot::channel();
    // Check identity binding through a pending call with a deliberately wrong project.
    a.control_pending
        .lock()
        .await
        .insert(id, control::Pending::fixture(p, "b", tx));
    assert_eq!(
        control::finish(
            &a,
            &Credential {
                project_id: q,
                client_id: "b".into(),
                role: "".into()
            },
            control::Reply {
                id,
                result: Some(json!({})),
                error: None
            }
        )
        .await,
        Err(StatusCode::FORBIDDEN)
    );
    assert!(a.control_pending.lock().await.contains_key(&id));
}

#[tokio::test]
async fn competing_edits_have_exactly_one_winner() {
    let store = policy_store::Store::memory();
    let p = Uuid::new_v4();
    store
        .put(p, "template", "default", 0, json!({"x":0}))
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        store.put(p, "template", "default", 1, json!({"x":1})),
        store.put(p, "template", "default", 1, json!({"x":2}))
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        store.get(p, "template", "default").await.unwrap().version,
        2
    );
}

#[tokio::test]
async fn relay_a2a_and_pmo_execute_real_member_dispatch() {
    let s = state("root").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["limited", "leader", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("limited");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    let (tx, _) = mpsc::channel(128);
    let answer = policies::engine(&s, p, &policy, "task", &policies::Dispatch::new(&tx, &[]))
        .await
        .unwrap();
    assert_eq!(answer, "worker completed");
    policy.members[0] = Member {
        path: vec!["leader".into()],
        role: "Leader".into(),
    };
    policy.mode = Mode::A2a;
    policy.rounds = 2;
    logs.lock().await.clear();
    let (tx, _) = mpsc::channel(128);
    policies::engine(
        &s,
        p,
        &policy,
        "discuss",
        &policies::Dispatch::new(&tx, &[]),
    )
    .await
    .unwrap();
    assert_eq!(logs.lock().await.len(), 4);
    assert!(logs.lock().await[1].contains("leader completed"));
    policy.mode = Mode::Pmo;
    policy.leader = Some(vec!["leader".into()]);
    logs.lock().await.clear();
    let (tx, _) = mpsc::channel(128);
    policies::engine(&s, p, &policy, "plan", &policies::Dispatch::new(&tx, &[]))
        .await
        .unwrap();
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 3);
    assert!(logs[1].contains("do research"));
    assert!(logs[2].contains("worker completed"));
}

#[tokio::test]
async fn a2a_round_two_flags_each_speaker_own_messages() {
    let s = state("a2a-self").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["leader", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("leader");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    policy.mode = Mode::A2a;
    policy.rounds = 2;
    let (tx, _) = mpsc::channel(128);
    policies::engine(
        &s,
        p,
        &policy,
        "discuss",
        &policies::Dispatch::new(&tx, &[]),
    )
    .await
    .unwrap();
    let logs = logs.lock().await;
    // Dispatch order: round1 leader, round1 worker, round2 leader, round2 worker.
    assert_eq!(logs.len(), 4);
    let round2_leader = &logs[2];
    assert!(round2_leader.contains("You (leader"));
    assert!(round2_leader.contains("worker: worker completed"));
    // The leader's own round-1 statement is not presented as another participant's line.
    assert!(!round2_leader.contains("\nleader: leader completed"));
    let round2_worker = &logs[3];
    assert!(round2_worker.contains("You (worker"));
    assert!(round2_worker.contains("leader: leader completed"));
}

#[tokio::test]
async fn discussion_stops_on_consensus_rechecks_new_opinions_and_yields_by_role() {
    for (replies, expected, unclaimed) in [
        (
            vec![
                vec!["方案一"],
                vec!["同意当前结论，无补充。"],
                vec!["同意当前结论，无补充。"],
            ],
            3,
            false,
        ),
        (
            vec![
                vec!["方案一", "同意当前结论，无补充。"],
                vec!["同意当前结论，无补充。", "同意当前结论，无补充。"],
                vec!["改为方案二"],
            ],
            5,
            false,
        ),
        (
            vec![
                vec![
                    "本轮让出。",
                    "本轮让出。原因：这是其他成员的职责，我没有待处理事项。",
                    "本轮让出。",
                    "本轮让出。原因：新意见仍不属于我的职责，未被指派。",
                ],
                vec!["方案一"],
                vec!["同意当前结论，无补充。"],
            ],
            6,
            false,
        ),
        (
            vec![
                vec!["本轮让出。", "本轮让出。原因：任务与职责无关，也未被指派。"],
                vec!["本轮让出。", "本轮让出。原因：任务与职责无关，也未被指派。"],
                vec!["本轮让出。", "本轮让出。原因：任务与职责无关，也未被指派。"],
            ],
            6,
            true,
        ),
    ] {
        let s = state("discussion").await;
        let p = Uuid::new_v4();
        let logs = Arc::new(Mutex::new(vec![]));
        let mut policy = relay("a");
        policy.mode = Mode::A2a;
        policy.rounds = 60;
        policy.members = ["a", "b", "c"]
            .into_iter()
            .map(|id| Member {
                path: vec![id.into()],
                role: format!("{id} 的职责"),
            })
            .collect();
        for (member, replies) in policy.members.iter().zip(replies) {
            scripted_executor(&s, p, &member.path[0], logs.clone(), replies).await;
        }
        let (tx, _) = mpsc::channel(128);
        let result = policies::engine(&s, p, &policy, "任务", &policies::Dispatch::new(&tx, &[]))
            .await
            .unwrap();
        let logs = logs.lock().await;
        assert_eq!(
            logs.len(),
            expected,
            "must stop without dispatching another member"
        );
        assert_eq!(result.contains("没有成员认领"), unclaimed);
        assert!(logs.iter().all(
            |prompt| prompt.contains("first check the user's latest request")
                && prompt.contains("do not call tools")
        ));
    }
}

#[tokio::test]
async fn discussion_rechecks_yields_and_rejects_addressed_abstentions_without_looping() {
    for (addressed, replies, expected_error) in [
        (false, vec!["本轮让出。", "已根据你的追问完成检查。"], None),
        (
            false,
            vec!["本轮让出。", "本轮让出。"],
            Some("未提供有效理由"),
        ),
        (
            true,
            vec!["本轮让出。", "本轮让出。原因：我的默认角色不负责这件事。"],
            Some("用户已明确指派"),
        ),
        (
            true,
            vec!["本轮让出。", "无法访问该文件，请提供可读路径。"],
            None,
        ),
    ] {
        let s = state("participation").await;
        let p = Uuid::new_v4();
        let logs = Arc::new(Mutex::new(vec![]));
        scripted_executor(&s, p, "a", logs.clone(), replies).await;
        scripted_executor(&s, p, "b", logs.clone(), vec!["同意当前结论，无补充。"]).await;
        let mut policy = relay("a");
        policy.mode = Mode::A2a;
        policy.rounds = 60;
        policy.members.push(Member {
            path: vec!["b".into()],
            role: "review".into(),
        });
        let (tx, _) = mpsc::channel(128);
        let paths = if addressed {
            vec![vec!["a".into()], vec!["b".into()]]
        } else {
            vec![]
        };
        let dispatch = policies::Dispatch {
            addressed: &paths,
            ..policies::Dispatch::new(&tx, &[])
        };
        let result =
            policies::engine(&s, p, &policy, "我已经明确要求你检查，请继续。", &dispatch).await;
        let logs = logs.lock().await;
        if let Some(error) = expected_error {
            assert!(result.unwrap_err().contains(error));
            assert_eq!(
                logs.len(),
                2,
                "stop after one recheck, never skip to the next member"
            );
        } else {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(logs.len(), 3);
        }
        assert!(
            logs[0].contains("take precedence over default roles")
                || logs[0].contains("must not override the user's assignment")
        );
        assert!(logs[1].contains("Responsibility recheck"));
        assert!(logs[1].contains("Do not repeat operations already performed"));
    }
}

#[tokio::test]
async fn single_addressed_member_cannot_escape_by_switching_to_chat_mode() {
    let s = state("direct-participation").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    scripted_executor(
        &s,
        p,
        "a",
        logs.clone(),
        vec!["本轮让出。", "本轮让出。原因：应该其他人先处理。"],
    )
    .await;
    let mut policy = relay("a");
    policy.mode = Mode::A2a;
    policy.members.push(Member {
        path: vec!["b".into()],
        role: "review".into(),
    });
    let paths = vec![vec!["a".into()]];
    let policy = crate::core::mentions::narrow(&policy, &paths)
        .unwrap()
        .unwrap();
    assert_eq!(policy.mode, Mode::A2a);
    let (tx, _) = mpsc::channel(128);
    let dispatch = policies::Dispatch {
        addressed: &paths,
        ..policies::Dispatch::new(&tx, &[])
    };
    let error = policies::engine(&s, p, &policy, "请你处理", &dispatch)
        .await
        .unwrap_err();
    assert!(error.contains("用户已明确指派"));
    assert_eq!(logs.lock().await.len(), 2);
}

#[tokio::test]
async fn addressed_owner_can_consult_a_peer_and_resume_with_the_answer() {
    let s = state("consultation").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    scripted_executor(
        &s,
        p,
        "a",
        logs.clone(),
        vec![
            "@b Which API version should I use?",
            "Implemented the task using API v2.",
        ],
    )
    .await;
    scripted_executor(&s, p, "b", logs.clone(), vec!["Use API v2; v1 is retired."]).await;
    let mut policy = relay("a");
    policy.mode = Mode::A2a;
    policy.members.push(Member {
        path: vec!["b".into()],
        role: "API specialist".into(),
    });
    policy.members.push(Member {
        path: vec!["c".into()],
        role: "unneeded peer".into(),
    });
    let paths = vec![vec!["a".into()]];
    let policy = crate::core::mentions::narrow(&policy, &paths)
        .unwrap()
        .unwrap();
    assert_eq!(policy.members.len(), 3);
    let (tx, _) = mpsc::channel(128);
    let dispatch = policies::Dispatch {
        addressed: &paths,
        ..policies::Dispatch::new(&tx, &[])
    };
    let result = policies::engine(&s, p, &policy, "@a implement the feature", &dispatch)
        .await
        .unwrap();
    let logs = logs.lock().await;
    assert_eq!(
        logs.iter()
            .map(|p| p.split(':').next().unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "b", "a"]
    );
    assert!(logs[1].contains("Peer @a asks you"));
    assert!(logs[1].contains("NOT a new human instruction or authorization"));
    assert!(logs[2].contains("Use API v2; v1 is retired."));
    assert!(logs[2].contains("retain responsibility"));
    assert!(result.contains("b: Use API v2"));
    assert!(result.contains("a: Implemented the task"));
}

#[tokio::test]
async fn every_mode_states_each_members_own_identity() {
    for mode in [Mode::Chat, Mode::Relay, Mode::A2a, Mode::Pmo] {
        let s = state("identity").await;
        let p = Uuid::new_v4();
        let logs = Arc::new(Mutex::new(vec![]));
        let mut policy = relay("leader");
        // Chat mode is single-member by contract, so the roster cannot be widened.
        if mode != Mode::Chat {
            executor(&s, p, "worker", logs.clone()).await;
            policy.members.push(Member {
                path: vec!["worker".into()],
                role: "researcher".into(),
            });
        }
        executor(&s, p, "leader", logs.clone()).await;
        policy.mode = mode.clone();
        policy.leader = Some(vec!["leader".into()]);
        policy.rounds = 1;
        let (tx, _) = mpsc::channel(128);
        policies::engine(
            &s,
            p,
            &policy,
            "discuss",
            &policies::Dispatch::new(&tx, &[]),
        )
        .await
        .unwrap();
        let logs = logs.lock().await;
        assert!(!logs.is_empty(), "{mode:?} dispatched nobody");
        for entry in logs.iter() {
            let speaker = entry.split(':').next().unwrap();
            assert!(
                entry.contains(&format!("You are Agent \"{speaker}\" in this group")),
                "{mode:?} never told {speaker} who it is: {entry}"
            );
        }
    }
}

#[tokio::test]
async fn pmo_leader_is_shown_its_own_assignments_before_summarizing() {
    let s = state("pmo-self").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["leader", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("leader");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    policy.mode = Mode::Pmo;
    policy.leader = Some(vec!["leader".into()]);
    let (tx, _) = mpsc::channel(128);
    policies::engine(&s, p, &policy, "plan", &policies::Dispatch::new(&tx, &[]))
        .await
        .unwrap();
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 3);
    // The worker is told who assigned it.
    assert!(logs[1].contains("Leader assignment from leader"));
    // The leader is shown its own instruction as its own, not as fresh input.
    let summarize = &logs[2];
    assert!(summarize.contains("You (leader, your own assignment)"));
    assert!(summarize.contains("worker: do research"));
    assert!(summarize.contains("worker: worker completed"));
}

/// Turn 2+ replays the previous turn's records into every member's prompt. Without a
/// per-reader marker each Agent cannot tell which of those records it wrote itself.
#[tokio::test]
async fn prior_group_records_are_marked_as_self_for_their_author() {
    let s = state("prior-self").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["leader", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("leader");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    policy.leader = Some(vec!["leader".into()]);
    policy.mode = Mode::Pmo;
    s.policy_store
        .put(
            p,
            "group",
            "g1",
            0,
            json!({
                "name":"g","auto_name":false,"policy":policy,
                "workspace":Value::Null,"status":"ready"
            }),
        )
        .await
        .unwrap();
    for content in ["first", "second"] {
        logs.lock().await.clear();
        policies::begin_group(
            &s,
            policies::RunRequest {
                project_id: p,
                group_id: "g1".into(),
                content: content.into(),
                previous_session_id: None,
                mentions: vec![],
                prompt: None,
            },
        )
        .await
        .unwrap();
        // A group run occupies its slot until the terminal event lands, so the next turn
        // must wait for it instead of being rejected as concurrent.
        for _ in 0..250 {
            let busy = s
                .store
                .list("runs")
                .await
                .iter()
                .any(|r| matches!(r["status"].as_str(), Some("queued" | "running")));
            if !busy && logs.lock().await.len() >= 3 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        if content == "second" {
            let logs = logs.lock().await;
            assert_eq!(logs.len(), 3, "turn 2 dispatched {}/3 members", logs.len());
            for entry in logs.iter() {
                let speaker = entry.split(':').next().unwrap();
                let replayed = entry
                    .split_once("Previous topic records from this group")
                    .and_then(|(_, rest)| rest.split('\n').nth(1))
                    .unwrap_or_else(|| panic!("{speaker} got no prior records: {entry}"));
                let records: Vec<Value> = serde_json::from_str(replayed).unwrap();
                let spoken = records
                    .iter()
                    .filter(|r| r["agent"].is_string() && r["type"] == "agent.message")
                    .collect::<Vec<_>>();
                assert!(
                    spoken
                        .iter()
                        .any(|r| r["agent"] == json!(speaker) && r["self"] == json!(true)),
                    "{speaker} was not told which records are its own: {replayed}"
                );
                assert!(
                    spoken
                        .iter()
                        .any(|r| r["agent"] != json!(speaker) && r["self"] == json!(false)),
                    "{speaker} cannot tell the other member's records apart: {replayed}"
                );
                // The human's turn and the group aggregate stay unattributed, so no member
                // can mistake them for something it said.
                assert!(
                    records
                        .iter()
                        .any(|r| r["agent"].is_null() && r["self"] == json!(false))
                );
            }
        }
    }
}

/// Run one turn and wait for it, so a following turn sees a settled group.
async fn turn(s: &AppState, p: Uuid, key: &str, content: &str, mentions: Vec<Vec<String>>) {
    policies::begin_group(
        s,
        policies::RunRequest {
            project_id: p,
            group_id: key.into(),
            content: content.into(),
            previous_session_id: None,
            mentions,
            // These tests exercise dispatch narrowing, not address stripping, which
            // `group_chat` does before it gets here.
            prompt: None,
        },
    )
    .await
    .unwrap();
    for _ in 0..250 {
        if !s
            .store
            .list("runs")
            .await
            .iter()
            .any(|r| matches!(r["status"].as_str(), Some("queued" | "running")))
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
/// Register a project the way a real node does, then place `policy` in a group.
/// `group_chat` rejects an unknown project before it ever looks at the message.
async fn group(s: &AppState, p: Uuid, policy: Policy) {
    s.store
        .insert(
            "projects",
            &p.to_string(),
            json!({"id":p,"space_id":Uuid::new_v4(),"name":"fixture","created_at":storage::now()}),
        )
        .await
        .unwrap();
    s.policy_store
        .put(
            p,
            "group",
            "g1",
            0,
            json!({
                "name":"g","auto_name":false,"policy":policy,
                "workspace":Value::Null,"status":"ready"
            }),
        )
        .await
        .unwrap();
}
#[tokio::test]
async fn addressing_one_member_dispatches_only_to_that_agent() {
    let s = state("at-single").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["alice", "carol"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("alice");
    policy.members.push(Member {
        path: vec!["carol".into()],
        role: "reviewer".into(),
    });
    group(&s, p, policy).await;

    let core = Core::new(s.clone());
    core.group_chat(
        p,
        json!({"group_id":"g1","content":"@carol 看一下这个错误"}),
    )
    .await
    .unwrap();
    for _ in 0..250 {
        if !s
            .store
            .list("runs")
            .await
            .iter()
            .any(|r| matches!(r["status"].as_str(), Some("queued" | "running")))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 1, "the unaddressed member ran: {logs:?}");
    assert!(logs[0].starts_with("carol:"), "{logs:?}");
    // The Agent must read the request, and be told the human chose it.
    assert!(logs[0].contains("看一下这个错误"), "{logs:?}");
    // The request itself no longer carries the address; the note explains why it ran.
    assert!(
        !logs[0].contains("@carol 看一下这个错误"),
        "the mention was left in the request: {logs:?}"
    );
    assert!(
        logs[0].contains("The human assigned this turn to @carol"),
        "{logs:?}"
    );
}
#[tokio::test]
async fn addressing_nothing_keeps_the_whole_group_running() {
    let s = state("at-none").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["alice", "carol"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("alice");
    policy.members.push(Member {
        path: vec!["carol".into()],
        role: "reviewer".into(),
    });
    policy.mode = Mode::A2a;
    policy.rounds = 2;
    group(&s, p, policy).await;

    // A `@name` that is not a member is prose, so the full group still answers.
    let core = Core::new(s.clone());
    core.group_chat(
        p,
        json!({"group_id":"g1","content":"ping @nobody about mail@example.com"}),
    )
    .await
    .unwrap();
    for _ in 0..250 {
        if !s
            .store
            .list("runs")
            .await
            .iter()
            .any(|r| matches!(r["status"].as_str(), Some("queued" | "running")))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    // Two members across two rounds.
    assert_eq!(logs.lock().await.len(), 4);
}
#[tokio::test]
async fn a_mention_only_message_is_rejected_rather_than_sent_empty() {
    let s = state("at-empty").await;
    let p = Uuid::new_v4();
    executor(&s, p, "alice", Arc::new(Mutex::new(vec![]))).await;
    group(&s, p, relay("alice")).await;
    let core = Core::new(s.clone());
    assert!(
        core.group_chat(p, json!({"group_id":"g1","content":"@alice"}))
            .await
            .is_err()
    );
    // A mention plus real content is fine.
    assert!(
        core.group_chat(p, json!({"group_id":"g1","content":"@alice 帮我看下"}))
            .await
            .is_ok()
    );
}
#[tokio::test]
async fn addressing_one_of_several_members_keeps_the_configured_mode() {
    let s = state("at-two").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["alice", "carol", "dave"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("alice");
    for id in ["carol", "dave"] {
        policy.members.push(Member {
            path: vec![id.into()],
            role: "peer".into(),
        });
    }
    policy.mode = Mode::A2a;
    group(&s, p, policy).await;

    let core = Core::new(s.clone());
    core.group_chat(
        p,
        json!({"group_id":"g1","content":"@carol @dave 一起看看"}),
    )
    .await
    .unwrap();
    for _ in 0..250 {
        if !s
            .store
            .list("runs")
            .await
            .iter()
            .any(|r| matches!(r["status"].as_str(), Some("queued" | "running")))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let logs = logs.lock().await;
    assert_eq!(logs.len(), 2, "alice ran despite not being named: {logs:?}");
    assert!(logs[0].starts_with("carol:") && logs[1].starts_with("dave:"));
}
#[tokio::test]
async fn a_name_outside_the_group_is_reported_instead_of_answered_by_everyone() {
    let s = state("at-stranger").await;
    let p = Uuid::new_v4();
    executor(&s, p, "alice", Arc::new(Mutex::new(vec![]))).await;
    group(&s, p, relay("alice")).await;
    // The group Chat mode holds exactly one member, so an unknown name must not
    // silently degrade into that member answering a stranger's request.
    let core = Core::new(s.clone());
    let result = core
        .group_chat(p, json!({"group_id":"g1","content":"@dave 在吗"}))
        .await;
    assert!(result.is_ok(), "@dave stays prose when it is not a member");
}
#[tokio::test]
async fn a_next_unaddressed_turn_still_reaches_the_whole_group() {
    let s = state("at-then-broad").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["alice", "carol"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("alice");
    policy.members.push(Member {
        path: vec!["carol".into()],
        role: "reviewer".into(),
    });
    policy.mode = Mode::A2a;
    group(&s, p, policy).await;

    // Narrowing must not mutate the stored policy.
    turn(&s, p, "g1", "看一下这个错误", vec![vec!["carol".into()]]).await;
    assert_eq!(logs.lock().await.len(), 1);
    logs.lock().await.clear();
    turn(&s, p, "g1", "大家都看一下", vec![]).await;
    assert_eq!(logs.lock().await.len(), 2, "the roster did not come back");
    let stored = s.policy_store.get(p, "group", "g1").await.unwrap();
    assert_eq!(
        stored.body["policy"]["members"].as_array().unwrap().len(),
        2,
        "the saved policy was narrowed"
    );
}
#[tokio::test]
async fn a_pmo_plan_wrapped_in_prose_and_a_fence_still_dispatches() {
    let s = state("pmo-fenced").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["chatty", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("chatty");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    policy.mode = Mode::Pmo;
    policy.leader = Some(vec!["chatty".into()]);
    let (tx, _) = mpsc::channel(128);
    // The leader answers with a fenced plan wrapped in prose, which is what a model
    // asked for "ONLY JSON" tends to produce anyway.
    policies::engine(&s, p, &policy, "plan", &policies::Dispatch::new(&tx, &[]))
        .await
        .unwrap();
    let logs = logs.lock().await;
    assert_eq!(
        logs.len(),
        3,
        "the fenced plan was not dispatched: {logs:?}"
    );
    assert!(logs[1].contains("do research"), "the assignment was lost");
    assert!(logs[2].contains("worker completed"));
}

#[tokio::test]
async fn a_pmo_that_answers_in_prose_is_asked_once_more_and_recovers() {
    let s = state("pmo-retry").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["reluctant", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("reluctant");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    policy.mode = Mode::Pmo;
    policy.leader = Some(vec!["reluctant".into()]);
    let (tx, _) = mpsc::channel(128);
    policies::engine(&s, p, &policy, "plan", &policies::Dispatch::new(&tx, &[]))
        .await
        .expect("one retry must recover a prose answer");
    let logs = logs.lock().await;
    // First ask, the retry that names the reason, then the dispatched work.
    assert_eq!(logs.len(), 4, "{logs:?}");
    assert!(logs[1].contains("previous answer was rejected"), "{logs:?}");
    assert!(logs[2].contains("do research"), "{logs:?}");
}

#[tokio::test]
async fn a_pmo_that_keeps_refusing_fails_after_exactly_one_retry() {
    let s = state("pmo-stubborn").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["stubborn", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("stubborn");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "researcher".into(),
    });
    policy.mode = Mode::Pmo;
    policy.leader = Some(vec!["stubborn".into()]);
    let (tx, _) = mpsc::channel(128);
    let error = policies::engine(&s, p, &policy, "plan", &policies::Dispatch::new(&tx, &[]))
        .await
        .unwrap_err();
    assert!(
        error.contains("Leader must return JSON assignments"),
        "{error}"
    );
    let logs = logs.lock().await;
    // Two attempts, no third: the retry is bounded, and no work was dispatched.
    assert_eq!(logs.len(), 2, "the retry did not stop: {logs:?}");
}

#[tokio::test]
async fn relay_negotiates_before_dispatch_and_random_keeps_a_complete_order() {
    let s = state("relay-strategies").await;
    let p = Uuid::new_v4();
    let logs = Arc::new(Mutex::new(vec![]));
    for id in ["leader", "worker"] {
        executor(&s, p, id, logs.clone()).await;
    }
    let mut policy = relay("leader");
    policy.members.push(Member {
        path: vec!["worker".into()],
        role: "research".into(),
    });
    policy.relay_strategy = crate::core::relay::Strategy::Negotiated;
    let (tx, mut rx) = mpsc::channel(128);
    let answer = policies::engine(&s, p, &policy, "task", &policies::Dispatch::new(&tx, &[]))
        .await
        .unwrap();
    assert_eq!(answer, "worker completed");
    let calls = logs.lock().await.clone();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].contains("RELAY NEGOTIATION ONLY"));
    assert!(calls[2].starts_with("worker:"));
    // The engine already finished, so every chunk it sent is queued; the caller keeps
    // the sender alive, which is why draining cannot wait for a close.
    let mut output = String::new();
    while let Ok(text) = rx.try_recv() {
        output.push_str(&text);
    }
    assert!(output.contains("worker → leader"));
    policy.relay_strategy = crate::core::relay::Strategy::Random;
    logs.lock().await.clear();
    let (tx, mut rx) = mpsc::channel(128);
    policies::engine(&s, p, &policy, "task", &policies::Dispatch::new(&tx, &[]))
        .await
        .unwrap();
    assert_eq!(logs.lock().await.len(), 1);
    // The engine already finished, so every chunk it sent is queued; the caller keeps
    // the sender alive, which is why draining cannot wait for a close.
    let mut output = String::new();
    while let Ok(text) = rx.try_recv() {
        output.push_str(&text);
    }
    assert!(output.contains("leader → worker") || output.contains("worker → leader"));
}

#[test]
fn policies_reject_overlapping_subtrees_and_unbounded_rounds() {
    let mut p = relay("b");
    p.members.push(Member {
        path: vec!["b".into(), "c".into()],
        role: "".into(),
    });
    assert!(p.validate().is_err());
    let mut p = relay("b");
    p.rounds = 99;
    assert!(p.validate().is_err());
}
