//! End-to-end launch of the real OpenCode CLI through the public factory.
//! Opt-in: it needs the binary installed, a reachable model and network, so it never
//! runs in the default suite. Set `CRABOT_OPENCODE_E2E=1` and `OPENCODE_MODEL`.
use agent_runtime::config::{OpenCodeConfig, RuntimeConfig};
use agent_runtime::{RuntimeFactory, RuntimeKind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn config(binary: &str, model: &str, auto_approve: bool) -> OpenCodeConfig {
    OpenCodeConfig {
        environment: Default::default(),
        binary: binary.into(),
        model: model.into(),
        agent: String::new(),
        auto_approve,
        thinking: true,
        // Matches the runtime default: a scrubbed HOME cannot reach the shared service.
        standalone: true,
        workdir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    }
}

#[test]
fn the_factory_builds_opencode_and_keeps_managed_flags_out_of_the_launcher() {
    let runtime =
        RuntimeFactory::from_config(RuntimeConfig::OpenCode(config("/bin/echo", "", false)))
            .unwrap();
    assert_eq!(runtime.kind(), RuntimeKind::OpenCode);
    // Task framing, output format, approval and continuity belong to Crabot; a configured
    // launcher may not smuggle them in, or it would silently take over the run.
    for flag in ["--format", "--auto", "run", "--thinking", "--prompt"] {
        assert!(
            RuntimeFactory::from_config(RuntimeConfig::OpenCode(config(
                &format!("/bin/echo {flag}"),
                "",
                false
            )))
            .is_err(),
            "{flag} must stay under Crabot's control"
        );
    }
    // A model name is not a reserved flag and must still pass through.
    assert!(
        RuntimeFactory::from_config(RuntimeConfig::OpenCode(config(
            "/bin/echo",
            "opencode/space-bunny-free",
            false
        )))
        .is_ok()
    );
}

#[tokio::test]
async fn a_real_opencode_run_streams_text_and_returns_the_same_answer() {
    if std::env::var("CRABOT_OPENCODE_E2E").as_deref() != Ok("1") {
        eprintln!("skipped: set CRABOT_OPENCODE_E2E=1 to exercise the real OpenCode CLI");
        return;
    }
    let model = std::env::var("OPENCODE_MODEL").unwrap_or_default();
    let runtime = RuntimeFactory::from_config(RuntimeConfig::OpenCode(config(
        &std::env::var("OPENCODE_BIN").unwrap_or_else(|_| "opencode".into()),
        &model,
        false,
    )))
    .unwrap();
    let streamed = Arc::new(Mutex::new(String::new()));
    let sink = streamed.clone();
    let answer = runtime
        .run_events("reply with exactly: ok", &mut move |event| {
            if let agent_runtime::RuntimeEvent::TextDelta { text } = event {
                sink.lock().unwrap().push_str(&text);
            }
        })
        .await
        .unwrap();
    assert_eq!(answer.trim(), "ok", "unexpected answer from the real CLI");
    // The republished-part dedupe is the whole risk here: streamed deltas must sum to
    // exactly the final answer, with no repeated prefix and no dropped tail.
    assert_eq!(*streamed.lock().unwrap(), answer);
}
