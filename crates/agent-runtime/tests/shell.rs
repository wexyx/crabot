use agent_runtime::{
    skills::{ExecutionPolicy, SkillCatalog},
    tools::{ToolContext, ToolFactory, ToolSession},
    workspace,
};
use serde_json::json;

#[tokio::test]
async fn shell_reads_text_larger_than_the_retired_file_limit() {
    let root = tempfile::tempdir().unwrap();
    let content = "汉字 and text\n".repeat(20_000);
    std::fs::write(root.path().join("large.txt"), &content).unwrap();
    let registry = ToolFactory::create(
        ToolContext::new(
            Some(root.path().into()),
            SkillCatalog::default(),
            ExecutionPolicy::new("default".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let result = agent_runtime::permissions::PermissionMode::Full
        .scope(registry.execute(
            "shell",
            &json!({"command":"cat large.txt"}),
            &mut ToolSession::default(),
        ))
        .await
        .unwrap();
    assert_eq!(result["stdout"], content);
    assert_eq!(result["success"], true);
}

#[tokio::test]
async fn dynamic_commands_and_disabled_builtins_share_the_same_execution_gate() {
    let directory = tempfile::tempdir().unwrap();
    let policy=serde_json::from_value(json!({"disabled":["shell"],"external":[{"name":"workspace_check","description":"Check workspace","command":"printf checked","enabled":true}]})).unwrap();
    let context = ToolContext::new(
        Some(directory.path().into()),
        SkillCatalog::default(),
        ExecutionPolicy::new("default".into()).unwrap(),
    )
    .unwrap()
    .with_tool_policy(policy);
    let registry = ToolFactory::create(context).unwrap();
    assert!(!registry.definitions().iter().any(|d| d.name() == "shell"));
    assert!(
        registry
            .execute(
                "shell",
                &json!({"command":"printf bypass"}),
                &mut ToolSession::default()
            )
            .await
            .unwrap_err()
            .contains("disabled")
    );
    let task = tokio::spawn(async move {
        registry
            .execute("workspace_check", &json!({}), &mut ToolSession::default())
            .await
    });
    let root = directory.path().canonicalize().unwrap();
    let pending = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if let Some(r) = workspace::pending()
                .into_iter()
                .find(|r| r.workdir == root.to_string_lossy())
            {
                break r;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(pending.command.as_deref(), Some("printf checked"));
    workspace::decide(pending.id, false).unwrap();
    assert!(task.await.unwrap().unwrap_err().contains("rejected"));
    let collision=serde_json::from_value(json!({"external":[{"name":"shell","description":"shadow","command":"true","enabled":false}]})).unwrap();
    assert!(
        ToolFactory::create(
            ToolContext::new(
                Some(root),
                SkillCatalog::default(),
                ExecutionPolicy::new("default".into()).unwrap()
            )
            .unwrap()
            .with_tool_policy(collision)
        )
        .is_err()
    );
}

#[tokio::test]
async fn shell_requires_one_shot_approval_then_can_access_host_files() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(directory.path().join("outside"), "private").unwrap();
    let root = root.canonicalize().unwrap();
    let registry = ToolFactory::create(
        ToolContext::new(
            Some(root.clone()),
            SkillCatalog::default(),
            ExecutionPolicy::new("default".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(registry.definitions().iter().any(|d| d.name() == "shell"));
    assert!(
        registry
            .execute(
                "shell",
                &json!({"command":"echo bad\u{001b}"}),
                &mut ToolSession::default()
            )
            .await
            .is_err()
    );
    for (command, allow) in [
        ("printf approved > result", false),
        ("printf approved > result; cat result", true),
        ("cat ../outside", true),
    ] {
        let registry = registry.clone();
        let task = tokio::spawn(async move {
            registry
                .execute(
                    "shell",
                    &json!({"command":command}),
                    &mut ToolSession::default(),
                )
                .await
        });
        let approval = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if let Some(item) = workspace::pending()
                    .into_iter()
                    .find(|r| r.workdir == root.to_string_lossy())
                {
                    break item;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(approval.command.as_deref(), Some(command));
        if !allow {
            assert!(!root.join("result").exists());
        }
        workspace::decide(approval.id, allow).unwrap();
        assert!(workspace::decide(approval.id, true).is_err());
        let result = task.await.unwrap();
        if !allow {
            assert!(result.unwrap_err().contains("rejected"));
            assert!(!root.join("result").exists());
        } else if command.starts_with("printf") {
            let output = result.unwrap();
            assert_eq!(output["stdout"], "approved");
            assert_eq!(output["success"], true);
        } else {
            let output = result.unwrap();
            assert_eq!(output["success"], true);
            assert_eq!(output["stdout"], "private");
            assert_eq!(output["execution"], "host");
        }
        assert!(!workspace::pending().iter().any(|r| r.id == approval.id));
    }
}
