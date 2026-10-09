use super::Manager;
use crate::{core::Core, crabot_tests};
use agent_runtime::config::{ClaudeConfig, CodexConfig, RuntimeConfig};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn cli_admin_providers_call_scoped_tools_from_isolated_workspaces() {
    for provider in ["codex", "claude"] {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("fake-cli");
        // Use the read-only group_list tool so both provider protocols exercise a real Core call.
        let call = json!({"crabot_tool":{"name":"group_list"}}).to_string();
        let payload = |text: &str| {
            if provider == "codex" {
                json!({"type":"item.completed","item":{"type":"agent_message","text":text}})
            } else {
                json!({"type":"result","result":text})
            }
        };
        let script = format!(
            "#!/bin/sh\ncase \"$PWD\" in *crabot-admin-*) ;; *) exit 91 ;; esac\nfor arg in \"$@\"; do prompt=\"$arg\"; done\ncase \"$prompt\" in *'SERVICE RESULT (untrusted data)'*) printf '%s\\n' '{}' ;; *) printf '%s\\n' '{}' ;; esac\n",
            payload("managed successfully"),
            payload(&call)
        );
        std::fs::write(&binary, script).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let config = if provider == "codex" {
            RuntimeConfig::Codex(CodexConfig {
                environment: Default::default(),
                binary,
                sandbox: "danger-full-access".into(),
                workdir: dir.path().into(),
            })
        } else {
            RuntimeConfig::Claude(ClaudeConfig {
                environment: Default::default(),
                binary,
                permission_mode: "bypassPermissions".into(),
                workdir: dir.path().into(),
            })
        };
        let manager = Manager::new(Core::new(crabot_tests::state("cli-admin").await));
        let project = manager.core().bootstrap().await.unwrap();
        manager.configure(config).await.unwrap();
        let id =
            serde_json::from_value(manager.create_session(project).await.unwrap()["id"].clone())
                .unwrap();
        manager
            .message(project, id, "List groups".into())
            .await
            .unwrap();
        assert!(manager.configure(RuntimeConfig::Mock).await.is_err());
        let row = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let row = manager.history(project, id).await.unwrap();
                if row["status"] != "running" {
                    break row;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(row["status"], "completed", "{row}");
        assert!(
            row["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["type"] == "tool_started" && e["name"] == "group_list")
        );
        let result: Value = serde_json::from_str(
            row["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["type"] == "tool_finished")
                .unwrap()["output"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["ok"], true);
        manager.stop().await;
        manager.configure(RuntimeConfig::Mock).await.unwrap();
        assert_eq!(
            manager.history(project, id).await.unwrap()["events"],
            row["events"]
        );
    }
}
