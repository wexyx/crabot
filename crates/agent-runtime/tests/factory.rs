use agent_runtime::{
    RuntimeFactory, RuntimeKind,
    config::{HarnessConfig, ModelApi, RuntimeConfig},
    is_token_insufficient,
};

#[test]
fn crabot_name_and_legacy_alias_select_the_same_provider() {
    assert_eq!(
        "crabot".parse::<RuntimeKind>().unwrap(),
        RuntimeKind::Crabot
    );
    assert_eq!(
        "builtin".parse::<RuntimeKind>().unwrap(),
        RuntimeKind::Crabot
    );
}

#[tokio::test]
async fn mock_factory_is_reusable_and_legacy_entry_stays_compatible() {
    let runtime = RuntimeFactory::create("mock").unwrap();
    assert_eq!(runtime.kind(), RuntimeKind::Mock);
    for prompt in ["first", "second"] {
        let mut chunks = String::new();
        let answer = runtime
            .run(prompt, &mut |s| chunks.push_str(&s))
            .await
            .unwrap();
        assert_eq!(chunks, answer);
        let legacy = agent_runtime::agent::run_with_provider("mock", prompt, |_| {})
            .await
            .unwrap();
        assert_eq!(answer, legacy);
    }
}

#[test]
fn unknown_runtime_and_invalid_explicit_config_fail_at_factory() {
    assert!(
        matches!(RuntimeFactory::create("unknown"),Err(e) if e.contains("Unknown AGENT_PROVIDER"))
    );
    let config = HarnessConfig {
        environment: Default::default(),
        context: Default::default(),
        system_prompt: agent_runtime::config::default_crabot_system_prompt().into(),
        api: ModelApi::Chat,
        base: "file:///private".into(),
        key: "fixture".into(),
        model: "fixture".into(),
        max_tokens: Some(64),
        deepseek_effort: None,
        root: ".".into(),
    };
    assert!(RuntimeFactory::from_config(RuntimeConfig::Crabot(config)).is_err());
}

#[cfg(unix)]
mod cli {
    use super::*;
    use agent_runtime::config::{ClaudeConfig, CodexConfig};
    use std::{
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(output: &str) -> Self {
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "crabot-runtime-{}-{id}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            let script = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > arguments.txt\npwd > cwd.txt\n{output}\n"
            );
            std::fs::write(root.join("fake-cli"), script).unwrap();
            std::fs::set_permissions(
                root.join("fake-cli"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            Self(root)
        }
        fn runtime(&self, kind: RuntimeKind) -> Box<dyn agent_runtime::AgentRuntime> {
            self.runtime_command(kind, self.0.join("fake-cli"))
        }
        fn runtime_command(
            &self,
            kind: RuntimeKind,
            binary: std::path::PathBuf,
        ) -> Box<dyn agent_runtime::AgentRuntime> {
            let config = match kind {
                RuntimeKind::Claude => RuntimeConfig::Claude(ClaudeConfig {
                    environment: Default::default(),
                    binary,
                    permission_mode: "plan".into(),
                    workdir: self.0.clone(),
                }),
                RuntimeKind::Codex => RuntimeConfig::Codex(CodexConfig {
                    environment: Default::default(),
                    binary,
                    sandbox: "read-only".into(),
                    workdir: self.0.clone(),
                }),
                _ => unreachable!(),
            };
            RuntimeFactory::from_config(config).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn both_cli_launchers_accept_quoted_commands_and_keep_managed_flags() {
        for kind in [RuntimeKind::Claude, RuntimeKind::Codex] {
            let output = if kind == RuntimeKind::Claude {
                r#"printf '%s\n' '{"type":"result","result":"ok"}'"#
            } else {
                r#"printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"ok"}}'"#
            };
            let fixture = Fixture::new(output);
            let spaced = fixture.0.join("fake cli");
            std::fs::rename(fixture.0.join("fake-cli"), &spaced).unwrap();
            let command = format!("\"{}\" --model 'model with spaces'", spaced.display());
            assert_eq!(
                fixture
                    .runtime_command(kind, command.into())
                    .run("hello", &mut |_| {})
                    .await
                    .unwrap(),
                "ok"
            );
            let arguments = std::fs::read_to_string(fixture.0.join("arguments.txt")).unwrap();
            assert!(arguments.starts_with("--model\nmodel with spaces\n"));
            assert!(arguments.contains(if kind == RuntimeKind::Claude {
                "--output-format\nstream-json"
            } else {
                "exec\n--json"
            }));
            assert_eq!(arguments.lines().last(), Some("hello"));
        }
    }
    #[tokio::test]
    async fn cli_factories_preserve_streaming_flags_and_working_directory() {
        for (kind, output, flag) in [
            (
                RuntimeKind::Claude,
                r#"printf '%s\n' '{"type":"stream_event","event":{"delta":{"text":"hello"}}}' '{"type":"result","result":"hello"}'"#,
                "--permission-mode\nplan",
            ),
            (
                RuntimeKind::Codex,
                r#"printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"hello"}}'"#,
                "--sandbox\nread-only",
            ),
        ] {
            let fixture = Fixture::new(output);
            let runtime = fixture.runtime(kind);
            assert_eq!(runtime.kind(), kind);
            let mut chunks = String::new();
            let answer = runtime
                .run("a prompt; $not_a_shell_command", &mut |s| {
                    chunks.push_str(&s)
                })
                .await
                .unwrap();
            assert_eq!(answer, "hello");
            assert_eq!(chunks, "hello");
            let args = std::fs::read_to_string(fixture.0.join("arguments.txt")).unwrap();
            assert!(args.contains(flag));
            assert_eq!(args.lines().last(), Some("a prompt; $not_a_shell_command"));
            let cwd = std::fs::read_to_string(fixture.0.join("cwd.txt")).unwrap();
            assert_eq!(
                std::fs::canonicalize(cwd.trim()).unwrap(),
                std::fs::canonicalize(&fixture.0).unwrap()
            );
        }
    }
    #[tokio::test]
    async fn cli_factories_preserve_token_limit_classification() {
        for kind in [RuntimeKind::Claude, RuntimeKind::Codex] {
            let fixture = Fixture::new("printf 'context window exceeded' >&2\nexit 1");
            let error = fixture
                .runtime(kind)
                .run("task", &mut |_| {})
                .await
                .unwrap_err();
            assert!(is_token_insufficient(&error));
            assert!(error.starts_with("TOKEN_INSUFFICIENT:"));
        }
    }
    #[tokio::test]
    async fn ordinary_cli_failure_does_not_become_token_failure() {
        for kind in [RuntimeKind::Claude, RuntimeKind::Codex] {
            let fixture = Fixture::new("printf 'not authenticated' >&2\nexit 7");
            let error = fixture
                .runtime(kind)
                .run("task", &mut |_| {})
                .await
                .unwrap_err();
            assert!(error.contains("not authenticated"));
            assert!(!is_token_insufficient(&error));
        }
    }
    #[tokio::test]
    async fn process_exit_failure_overrides_a_success_payload() {
        use agent_runtime::{RuntimeErrorCode, RuntimeEvent};
        for (kind, payload) in [
            (
                RuntimeKind::Claude,
                r#"printf '%s\n' '{"type":"result","result":"partial"}'"#,
            ),
            (
                RuntimeKind::Codex,
                r#"printf '%s\n' '{"type":"turn.completed"}'"#,
            ),
        ] {
            let fixture = Fixture::new(&format!("{payload}\nprintf 'process failed' >&2\nexit 7"));
            let mut events = Vec::new();
            assert!(
                fixture
                    .runtime(kind)
                    .run_events("task", &mut |e| events.push(e))
                    .await
                    .is_err()
            );
            assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
            assert!(matches!(
                events.last(),
                Some(RuntimeEvent::Failed {
                    code: RuntimeErrorCode::ExecutionFailed,
                    ..
                })
            ));
        }
    }
}
