use super::*;
use crate::{
    skills::{ExecutionPolicy, SkillCatalog, SkillDefinition},
    tools::{ToolFactory, ToolPolicy, ToolSession},
};
use std::collections::BTreeMap;

fn skill(id: &str, enabled: bool) -> SkillDefinition {
    SkillDefinition::new(
        id.into(),
        format!("{id} summary"),
        BTreeMap::from([
            ("SKILL.md".into(), format!("{id} full instructions")),
            ("scripts/run.py".into(), "print('ok')".into()),
        ]),
        enabled,
        false,
    )
    .unwrap()
}

#[tokio::test]
async fn builtins_are_ready_on_first_turn_while_user_bodies_and_external_tools_are_discovered() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = SkillCatalog::new(vec![
        skill("builtin-demo", true),
        skill("user-demo", true),
        skill("disabled-demo", false),
    ])
    .unwrap()
    .with_builtins(["builtin-demo".into(), "disabled-demo".into()]);
    let policy: ToolPolicy =
        serde_json::from_value(json!({"disabled":["doc"],"external":[]})).unwrap();
    let registry = ToolFactory::create(
        ToolContext::new(
            Some(dir.path().into()),
            catalog,
            ExecutionPolicy::new("offline".into()).unwrap(),
        )
        .unwrap()
        .with_tool_policy(policy),
    )
    .unwrap();
    let advertised = registry
        .advertised(&ToolSession::default())
        .iter()
        .map(|d| d.name().to_owned())
        .collect::<Vec<_>>();
    for name in ["find", "shell", "compact", "skill"] {
        assert!(advertised.iter().any(|n| n == name), "{name}");
    }
    assert!(!advertised.iter().any(|n| n == "doc"));
    let prompt = registry.instructions().await.unwrap();
    assert!(prompt.contains("builtin-demo full instructions"));
    assert!(prompt.contains("user-demo summary"));
    assert!(!prompt.contains("user-demo full instructions"));
    assert!(!prompt.contains("disabled-demo"));
    let catalog_line = prompt.lines().find(|l| l.starts_with("Skills: ")).unwrap();
    let skills: serde_json::Value =
        serde_json::from_str(catalog_line.strip_prefix("Skills: ").unwrap()).unwrap();
    let builtin = skills
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "builtin-demo")
        .unwrap();
    let script =
        std::path::Path::new(builtin["directory"].as_str().unwrap()).join("scripts/run.py");
    assert_eq!(std::fs::read_to_string(script).unwrap(), "print('ok')");
    assert_eq!(
        prompt,
        registry.instructions().await.unwrap(),
        "same registry reuses materialized directories"
    );
    assert_eq!(
        crate::paths::memory_file(),
        crate::paths::data_dir().join("work/memory.md")
    );
    // A dynamically registered definition is not automatically a built-in.
    struct External;
    impl super::super::Tool for External {
        fn definition(&self) -> super::super::ToolDefinition {
            super::super::ToolDefinition::new("external_demo", "demo", json!({"type":"object"}))
        }
        fn execute<'a>(
            &'a self,
            _: &'a serde_json::Value,
            _: &'a mut ToolSession,
        ) -> super::super::ToolFuture<'a> {
            Box::pin(async { Ok(json!({})) })
        }
    }
    let mut registry = registry;
    registry.register(Arc::new(External)).unwrap();
    let mut session = ToolSession::default();
    assert!(
        !registry
            .advertised(&session)
            .iter()
            .any(|d| d.name() == "external_demo")
    );
    session.unlock("external_demo");
    assert!(
        registry
            .advertised(&session)
            .iter()
            .any(|d| d.name() == "external_demo")
    );
}
