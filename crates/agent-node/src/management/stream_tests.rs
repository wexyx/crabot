use super::{journal::Journal, session};
use crate::{core::Core, crabot_tests};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn append_log_before_final_metadata_and_reject_sequence_conflicts() {
    let core = Core::new(crabot_tests::state("stream-batch").await);
    let id = Uuid::new_v4();
    core.state()
        .store
        .insert(
            "management_sessions",
            &id.to_string(),
            json!({"id":id,"project_id":id,"status":"running"}),
        )
        .await
        .unwrap();
    let events = session::append_batch(
        &core,
        id,
        vec![
            json!({"seq":1,"type":"text_delta","text":"你好"}),
            json!({"type":"completed","text":"你好"}),
        ],
        Some("completed"),
    )
    .await
    .unwrap();
    let before = core
        .state()
        .store
        .get("management_sessions", &id.to_string())
        .await
        .unwrap();
    assert_eq!(before["status"], "completed");
    assert!(before.get("events").is_none());
    // The index is the only store, including streaming and terminal markers.
    let history = crate::core::indexed_history::IndexedHistory::new(id, "admin".into(), None);
    let (rows, _) = history.rows(None, 0, u64::MAX, 20).await.unwrap();
    assert_eq!(rows, events);
    assert!(
        session::append_batch(
            &core,
            id,
            vec![json!({"seq":1,"type":"text_delta","text":"duplicate"})],
            Some("running")
        )
        .await
        .is_err()
    );
    assert_eq!(
        core.state()
            .store
            .get("management_sessions", &id.to_string())
            .await
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn slow_subscriber_can_replay_the_live_snapshot_and_failure_is_visible() {
    let journal = Journal::default();
    let id = Uuid::new_v4();
    journal.begin(id, json!({"id":id,"events":[],"status":"running"}));
    let mut receiver = journal.subscribe();
    for _ in 0..2100 {
        journal
            .emit(id, json!({"type":"text_delta","text":"字"}))
            .unwrap();
    }
    assert!(matches!(
        receiver.recv().await,
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_))
    ));
    let snapshot = journal.history(id).unwrap();
    assert_eq!(snapshot["events"].as_array().unwrap().len(), 2100);
    journal.failed(id, "disk unavailable".into());
    let snapshot = journal.history(id).unwrap();
    assert_eq!(snapshot["status"], "failed");
    assert_eq!(snapshot["persistence_failed"], true);
    assert_eq!(
        snapshot["events"].as_array().unwrap().last().unwrap()["type"],
        "failed"
    );
}
