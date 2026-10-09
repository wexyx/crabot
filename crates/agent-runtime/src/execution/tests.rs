use super::{profile::Profile, service::ExecutionService};
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Fixture {
    entered: tokio::sync::Notify,
    dropped: Arc<AtomicUsize>,
    hang: bool,
}
struct Guard(Arc<AtomicUsize>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl Fixture {
    async fn execute(self: Arc<Self>, _profile: Profile) -> Result<serde_json::Value, String> {
        let _guard = Guard(self.dropped.clone());
        self.entered.notify_one();
        if self.hang {
            std::future::pending::<()>().await;
        }
        Ok(json!({"stdout":"ok"}))
    }
}
async fn run(
    execution: &Arc<ExecutionService>,
    fixture: Arc<Fixture>,
    profile: String,
) -> Result<serde_json::Value, String> {
    execution
        .supervise(profile, move |profile| fixture.execute(profile))
        .await
}
fn setup(hang: bool) -> (Arc<ExecutionService>, Arc<Fixture>, String) {
    let fixture = Arc::new(Fixture {
        entered: tokio::sync::Notify::new(),
        dropped: Arc::new(AtomicUsize::new(0)),
        hang,
    });
    let profile =
        serde_json::from_value(json!({"id":"default","network":"host","timeout_seconds":1}))
            .unwrap();
    let execution = Arc::new(ExecutionService::new(vec![profile]).unwrap());
    (execution, fixture, "default".into())
}
#[tokio::test]
async fn native_supervisor_cancellation_releases_execution_before_shutdown() {
    let (execution, fixture, request) = setup(true);
    let task = {
        let s = execution.clone();
        let fixture = fixture.clone();
        tokio::spawn(async move { run(&s, fixture, request).await })
    };
    tokio::time::timeout(Duration::from_secs(2), fixture.entered.notified())
        .await
        .unwrap();
    task.abort();
    execution.shutdown().await;
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn native_supervisor_timeout_and_shutdown_fail_closed() {
    let (execution, fixture, request) = setup(true);
    assert!(
        run(&execution, fixture.clone(), request.clone())
            .await
            .unwrap_err()
            .contains("timed out")
    );
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
    execution.shutdown().await;
    assert!(run(&execution, fixture.clone(), request).await.is_err());
}
#[tokio::test]
async fn native_supervisor_success_and_unknown_profile() {
    let (execution, fixture, request) = setup(false);
    assert_eq!(
        run(&execution, fixture.clone(), request.clone())
            .await
            .unwrap()["stdout"],
        "ok"
    );
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
    let invalid = "unknown".into();
    assert!(run(&execution, fixture.clone(), invalid).await.is_err());
}
#[test]
fn native_profile_rejects_retired_fields_and_unsafe_environment() {
    let base = json!({"id":"default","network":"host","timeout_seconds":30});
    let profile: Profile = serde_json::from_value(base.clone()).unwrap();
    profile.validate().unwrap();
    let mut offline = profile.clone();
    offline.network = "none".into();
    assert!(
        offline
            .validate()
            .unwrap_err()
            .contains("no longer isolates")
    );
    for field in [
        "image",
        "runtime",
        "cpus",
        "memory_mb",
        "pids",
        "scratch_mb",
    ] {
        let mut input = base.clone();
        input[field] = json!("retired");
        assert!(serde_json::from_value::<Profile>(input).is_err());
    }
    for name in [
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
        "PYTHONPATH",
        "PATH",
        "HOME",
    ] {
        let mut input = base.clone();
        input["secret_env"] = json!([name]);
        assert!(
            serde_json::from_value::<Profile>(input)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_kills_spawned_children() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let task = tokio::spawn(async move {
        let profile =
            serde_json::from_value(json!({"id":"default","network":"host","timeout_seconds":10}))
                .unwrap();
        super::native::shell::execute(
            root,
            "(sleep 1; printf leaked > leaked) & printf ready > ready; wait".into(),
            profile,
            Default::default(),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !dir.path().join("ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(!dir.path().join("leaked").exists());
}
