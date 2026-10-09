use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn owner(dir: &std::path::Path) -> SessionScope {
    SessionScope::new(
        uuid::Uuid::new_v4().to_string(),
        "chat".into(),
        "agent".into(),
    )
    .with_data(dir.join("data"))
}
async fn start(
    m: &ProcessSessions,
    s: &SessionScope,
    root: &std::path::Path,
    script: &str,
    pty: bool,
) -> Value {
    m.start(
        s.clone(),
        root.into(),
        script.into(),
        pty,
        BTreeMap::new(),
        None,
        100,
    )
    .await
    .unwrap()
}
async fn finished(m: &ProcessSessions, s: &SessionScope, id: &str) -> Value {
    m.read(s, id, 0, 3000).await.unwrap()
}

#[tokio::test]
async fn pipe_survives_return_and_accepts_input_with_incremental_output() {
    let dir = tempfile::tempdir().unwrap();
    let s = owner(dir.path());
    let m = ProcessSessions::new();
    let r = start(
        &m,
        &s,
        dir.path(),
        "printf ready; read value; printf 'got:%s' \"$value\"; printf err >&2",
        false,
    )
    .await;
    assert_eq!(r["status"], "running");
    assert_eq!(r["stdout"], "ready");
    let id = r["session_id"].as_str().unwrap();
    m.human_write(s.project(), s.chat(), id, "hello\n", false)
        .await
        .unwrap();
    let r = finished(&m, &s, id).await;
    assert_eq!(r["stdout"], "readygot:hello");
    assert_eq!(r["stderr"], "err");
    assert_eq!(r["success"], true);
    assert_eq!(
        m.read(&s, id, r["next_cursor"].as_u64().unwrap(), 0)
            .await
            .unwrap()["stdout"],
        ""
    );
    assert!(
        m.human_write(s.project(), s.chat(), id, "x", false)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn pty_has_controlling_terminal_and_preserves_shell_state() {
    let dir = tempfile::tempdir().unwrap();
    let s = owner(dir.path());
    let m = ProcessSessions::new();
    let r = start(
        &m,
        &s,
        dir.path(),
        "test -t 0 && test -t 1 && printf tty; exec /bin/sh -i",
        true,
    )
    .await;
    let id = r["session_id"].as_str().unwrap();
    assert!(r["stdout"].as_str().unwrap().contains("tty"));
    m.human_write(
        s.project(),
        s.chat(),
        id,
        "export SESSION_VAR=retained; cd /;\n",
        false,
    )
    .await
    .unwrap();
    m.human_write(
        s.project(),
        s.chat(),
        id,
        "printf 'value:%s dir:%s' \"$SESSION_VAR\" \"$PWD\"; exit\n",
        false,
    )
    .await
    .unwrap();
    let r = finished(&m, &s, id).await;
    assert!(
        r["stdout"]
            .as_str()
            .unwrap()
            .contains("value:retained dir:/"),
        "{r}"
    );
    assert_eq!(r["success"], true);
}
#[tokio::test]
async fn credentials_persist_for_agent_but_processes_are_chat_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let s = owner(dir.path());
    let m = ProcessSessions::new();
    let first = start(
        &m,
        &s,
        dir.path(),
        "printf credential > \"$HOME/token\"",
        false,
    )
    .await;
    finished(&m, &s, first["session_id"].as_str().unwrap()).await;
    let other = SessionScope::new(s.project().into(), "other-chat".into(), s.agent().into())
        .with_data(dir.path().join("data"));
    assert!(
        m.read(&other, first["session_id"].as_str().unwrap(), 0, 0)
            .await
            .is_err()
    );
    let second = start(&m, &other, dir.path(), "cat \"$HOME/token\"", false).await;
    assert_eq!(second["stdout"], "credential");
    let other_agent = SessionScope::new(s.project().into(), s.chat().into(), "other-agent".into())
        .with_data(dir.path().join("data"));
    let third = start(
        &m,
        &other_agent,
        dir.path(),
        "test ! -f \"$HOME/token\"",
        false,
    )
    .await;
    assert_eq!(third["success"], true);
    assert!(
        m.human_read("wrong", s.chat(), first["session_id"].as_str().unwrap(), 0)
            .is_err()
    );
}
#[tokio::test]
async fn private_input_hides_output_from_model_even_if_child_echoes_it() {
    let dir = tempfile::tempdir().unwrap();
    let s = owner(dir.path());
    let m = ProcessSessions::new();
    let r = start(
        &m,
        &s,
        dir.path(),
        "read token; printf 'secret:%s' \"$token\"",
        true,
    )
    .await;
    let id = r["session_id"].as_str().unwrap();
    m.human_write(s.project(), s.chat(), id, "password\n", true)
        .await
        .unwrap();
    let model = finished(&m, &s, id).await;
    assert_eq!(model["stdout"], "");
    assert_eq!(model["private_output"], true);
    assert!(!model.to_string().contains("password"));
    let human = m.human_read(s.project(), s.chat(), id, 0).unwrap();
    assert!(
        human["stdout"]
            .as_str()
            .unwrap()
            .contains("secret:password")
    );
}
#[tokio::test]
async fn model_input_requires_approval_even_for_a_whitelisted_word() {
    let dir = tempfile::tempdir().unwrap();
    let s = owner(dir.path());
    let m = std::sync::Arc::new(ProcessSessions::new());
    let r = start(
        &m,
        &s,
        dir.path(),
        "read input; printf '%s' \"$input\"",
        false,
    )
    .await;
    let id = r["session_id"].as_str().unwrap().to_owned();
    let correlation = uuid::Uuid::new_v4();
    let (m2, s2, id2) = (m.clone(), s.clone(), id.clone());
    let task = tokio::spawn(crate::workspace::with_approval_context(
        correlation,
        crate::permissions::PermissionMode::Auto
            .scope(async move { m2.write(&s2, &id2, "pwd\n").await }),
    ));
    let approval = loop {
        if let Some(p) = crate::workspace::pending()
            .into_iter()
            .find(|p| p.correlation_id == Some(correlation))
        {
            break p;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    crate::workspace::decide(approval.id, false).unwrap();
    assert!(task.await.unwrap().is_err());
    assert_eq!(m.read(&s, &id, 0, 0).await.unwrap()["status"], "running");
    m.stop(&s, &id).await.unwrap();
}
#[tokio::test]
async fn process_limits_and_shutdown_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let s = owner(dir.path());
    let m = ProcessSessions::new();
    for _ in 0..4 {
        start(&m, &s, dir.path(), "read line", false).await;
    }
    assert!(
        m.start(
            s.clone(),
            dir.path().into(),
            "true".into(),
            false,
            BTreeMap::new(),
            None,
            0
        )
        .await
        .is_err()
    );
    m.stop_chat(s.project(), s.chat());
    m.shutdown().await;
    assert!(
        m.list(s.project(), s.chat())
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["status"] != "running")
    );
}
#[test]
fn output_is_bounded_and_reports_lost_history() {
    let mut output = output::Output::new();
    for _ in 0..300 {
        output.append(false, &vec![b'x'; 4096]);
    }
    let r = output.view(0, false);
    assert_eq!(r["truncated"], true);
    assert!(r["stdout"].as_str().unwrap().len() <= 1024 * 1024);
    output.finish("completed", Some(3));
    assert_eq!(output.view(0, false)["success"], false);
    assert_eq!(json!(r["next_cursor"]), 300);
}
