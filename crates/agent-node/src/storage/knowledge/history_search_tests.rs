use super::*;

#[test]
fn searches_chats_and_projects_but_not_other_instances_and_reads_exact_hits() {
    let instance = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    for (root, project, chats) in [
        (instance.path(), a, vec!["first", "second"]),
        (instance.path(), b, vec!["third"]),
        (other.path(), Uuid::new_v4(), vec!["secret"]),
    ] {
        let db = graph::Knowledge::at(&root.join("knowledge").join(project.to_string())).unwrap();
        for chat in chats {
            super::super::ingest::persist_at(
                &db,
                chat,
                &[json!({"type":"user","content":format!("deployment {chat}")})],
            )
            .unwrap();
        }
    }
    let search = HistorySearch::new(instance.path());
    let query = || HistoryQuery {
        query: "deployment".into(),
        limit: 10,
        ..Default::default()
    };
    let result = search.search(query()).unwrap();
    assert_eq!(result["matched"], 3);
    assert!(!result.to_string().contains("secret"));
    assert_eq!(result["truncated"], false);
    let hit = &result["matches"][0];
    let rows = search
        .read(
            hit["project"].as_str().unwrap(),
            hit["chat"].as_str().unwrap(),
            0,
            u64::MAX,
            10,
        )
        .unwrap();
    assert!(
        rows["records"][0]["content"]
            .as_str()
            .unwrap()
            .contains(hit["chat"].as_str().unwrap())
    );
    assert_eq!(
        search
            .search(HistoryQuery {
                project: Some(b.to_string()),
                ..query()
            })
            .unwrap()["matched"],
        1
    );
    assert_eq!(
        search
            .search(HistoryQuery {
                limit: 1,
                ..query()
            })
            .unwrap()["truncated"],
        true
    );
    for bad in ["../outside".to_owned(), Uuid::new_v4().to_string()] {
        assert!(search.read(&bad, "first", 0, u64::MAX, 10).is_err());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            other
                .path()
                .join("knowledge")
                .read_dir()
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
            instance
                .path()
                .join("knowledge")
                .join(Uuid::new_v4().to_string()),
        )
        .unwrap();
        assert_eq!(search.search(query()).unwrap()["matched"], 3);
    }
}
