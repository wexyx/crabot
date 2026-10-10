use super::*;

#[tokio::test]
async fn authored_skills_are_registered_versioned_persisted_and_cannot_self_enable() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::storage::open(dir.path()).await.unwrap();
    let source = SkillAuthoring::new(store.clone(), "business");
    let definition = json!({"id":"custom-demo","description":"test","enabled":false,"allow_python":false,"files":{"SKILL.md":"demo","scripts/run.py":"print(1)"}});
    let saved = source
        .call(json!({"action":"save","expected_version":0,"definition":definition}))
        .await
        .unwrap();
    let id = saved["id"].as_str().unwrap();
    assert_eq!(saved["version"], 1);
    let stored = store.get("capability_library", id).await.unwrap();
    let path = dir
        .path()
        .join(stored["skill_files_directory"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(path.join("SKILL.md")).unwrap(),
        "demo"
    );
    let read = source.call(json!({"action":"read","id":id})).await.unwrap();
    assert_eq!(read["definition"], definition);
    assert!(
        source
            .call(json!({"action":"list"}))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == id)
    );
    let mut changed = definition.clone();
    changed["files"]["SKILL.md"] = json!("updated");
    assert_eq!(
        source
            .call(json!({"action":"save","id":id,"expected_version":1,"definition":changed}))
            .await
            .unwrap()["version"],
        2
    );
    assert!(
        source
            .call(json!({"action":"save","id":id,"expected_version":1,"definition":changed}))
            .await
            .is_err()
    );
    changed["enabled"] = json!(true);
    assert!(
        source
            .call(json!({"action":"save","id":id,"expected_version":2,"definition":changed}))
            .await
            .is_err()
    );
    let management = SkillAuthoring::new(store.clone(), "management");
    assert!(
        management
            .call(json!({"action":"read","id":id}))
            .await
            .is_err()
    );
    assert!(source.call(json!({"action":"save","expected_version":0,"definition":{"id":"escape","description":"test","enabled":false,"files":{"SKILL.md":"test","../escape":"bad"}}})).await.is_err());
    let library = Library::new(store);
    let catalog = library
        .skills(
            "business",
            &super::super::Context::new(uuid::Uuid::new_v4(), "test"),
        )
        .await
        .unwrap();
    assert!(
        !catalog
            .definitions()
            .iter()
            .any(|s| s.id() == "custom-demo")
    );
}
