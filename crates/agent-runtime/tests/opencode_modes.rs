//! Exercises both OpenCode service modes through the real runtime, because the shared
//! mode's failure was a silent hang rather than an error, which no unit test would see.
//!
//! Opt-in like the other OpenCode live tests: `CRABOT_OPENCODE_E2E=1` and `OPENCODE_MODEL`.

use agent_runtime::config::{OpenCodeConfig, RuntimeConfig};
use agent_runtime::{RuntimeFactory, RuntimeKind};

async fn run(standalone: bool) -> Result<String, String> {
    let mut config = OpenCodeConfig::from_env();
    config.standalone = standalone;
    if config.model.is_empty() {
        config.model = "opencode/space-bunny-free".into();
    }
    let runtime = RuntimeFactory::from_config(RuntimeConfig::OpenCode(config))?;
    assert_eq!(runtime.kind(), RuntimeKind::OpenCode);
    let started = std::time::Instant::now();
    let answer = runtime.run("say hi", &mut |_chunk: String| {}).await;
    eprintln!(
        "standalone={standalone} ok={} took={:?}",
        answer.is_ok(),
        started.elapsed()
    );
    answer
}

#[tokio::test]
async fn both_service_modes_return_an_answer() {
    if std::env::var("CRABOT_OPENCODE_E2E").as_deref() != Ok("1") {
        eprintln!("skipped: set CRABOT_OPENCODE_E2E=1 to exercise the real OpenCode CLI");
        return;
    }
    let private = run(true).await;
    let shared = run(false).await;
    // Neither mode may hang: the shared path used to produce no output at all.
    assert!(
        private.is_ok(),
        "the private server returned nothing: {private:?}"
    );
    assert!(
        shared.is_ok(),
        "the shared service returned nothing: {shared:?}"
    );
}
