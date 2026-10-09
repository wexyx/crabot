use agent_runtime::{
    skills::{ExecutionPolicy, SkillCatalog, SkillDefinition},
    tools::{ToolContext, ToolFactory, ToolRegistry, ToolSession, tool},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

struct Echo;
#[tool(name = "test_echo", description = "Echo")]
impl Echo {
    fn new(_context: Arc<ToolContext>) -> Option<Self> {
        Some(Self)
    }
    async fn execute(&self, args: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        Ok(args.clone())
    }
}

fn context(catalog: SkillCatalog) -> ToolContext {
    ToolContext::new(
        None,
        catalog,
        ExecutionPolicy::new("offline".into()).unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn macro_factory_and_dynamic_registration_use_same_contract() {
    let mut registry = ToolFactory::create(context(SkillCatalog::default())).unwrap();
    let mut session = ToolSession::default();
    assert_eq!(
        registry
            .execute("test_echo", &json!({"a":1}), &mut session)
            .await
            .unwrap(),
        json!({"a":1})
    );
    assert!(
        registry
            .register(Arc::new(Echo))
            .unwrap_err()
            .contains("duplicate")
    );
    let snapshot = registry.clone();
    assert!(registry.unregister("test_echo"));
    assert!(
        registry
            .execute("test_echo", &json!({}), &mut session)
            .await
            .is_err()
    );
    assert!(
        snapshot
            .execute("test_echo", &json!({}), &mut session)
            .await
            .is_ok()
    );
    registry.register(Arc::new(Echo)).unwrap();
    assert!(
        registry
            .execute("test_echo", &json!([]), &mut session)
            .await
            .is_err()
    );
    let mut empty = ToolRegistry::new();
    empty.register(Arc::new(Echo)).unwrap();
    assert_eq!(empty.definitions()[0].name(), "test_echo");
}
#[tokio::test]
async fn minimal_tools_load_skill_packages_and_execute_python_through_shell() {
    let root = tempfile::tempdir().unwrap();
    let skill = SkillDefinition::new(
        "demo".into(),
        "demo".into(),
        BTreeMap::from([
            ("SKILL.md".into(), "Use shell after reading".into()),
            ("scripts/run.py".into(), "print('ok')".into()),
        ]),
        true,
        false,
    )
    .unwrap();
    let registry = ToolFactory::create(
        ToolContext::new(
            Some(root.path().into()),
            SkillCatalog::new(vec![skill]).unwrap(),
            ExecutionPolicy::new("default".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let names = registry
        .definitions()
        .iter()
        .map(|d| d.name().to_owned())
        .collect::<Vec<_>>();
    for removed in [
        "read",
        "read_file",
        "list_files",
        "command_run",
        "context_compact",
        "python_run",
        "browser_run",
        "skill_file",
        "history_read",
        "image_show",
        "skill_read",
    ] {
        assert!(
            !names.iter().any(|name| name == removed),
            "{removed} must not be registered"
        );
    }
    for required in ["shell", "find", "compact"] {
        assert!(names.iter().any(|n| n == required));
    }
    let mut session = ToolSession::default();
    let loaded = registry
        .execute("find", &json!({"target":"skill","id":"demo"}), &mut session)
        .await
        .unwrap();
    assert_eq!(
        loaded["skills"][0]["instructions"],
        "Use shell after reading"
    );
    let script = std::path::Path::new(loaded["skills"][0]["directory"].as_str().unwrap())
        .join("scripts/run.py");
    let command = format!(
        "python3 {}",
        shlex::try_quote(script.to_str().unwrap()).unwrap()
    );
    let output = agent_runtime::permissions::PermissionMode::Full
        .scope(registry.execute("shell", &json!({"command":command}), &mut session))
        .await
        .unwrap();
    assert_eq!(output["stdout"], "ok\n");
    assert!(
        registry
            .execute(
                "find",
                &json!({"target":"skill","id":"outside"}),
                &mut session
            )
            .await
            .is_err()
    );
    assert!(
        registry
            .execute(
                "find",
                &json!({"target":"skill","id":"demo","command":"evil"}),
                &mut session
            )
            .await
            .is_err()
    );
    let other = ToolFactory::create(context(SkillCatalog::default())).unwrap();
    assert!(!other.definitions().iter().any(|d| d.name() == "skill_read"));
}
