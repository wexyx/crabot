use super::{DocumentStore, documents, graph::Knowledge};
use serde_json::json;

#[test]
fn lists_all_documents_by_import_time_with_stable_pages_and_filtered_totals() {
    let dir = tempfile::tempdir().unwrap();
    let db = Knowledge::at(dir.path()).unwrap();
    let mut ids = vec![];
    for index in 0..7 {
        let doc = documents::execute(&db, &json!({"action":"save","title":format!("Article {index}"),"content":if index % 2 == 0 { "even" } else { "odd" }})).unwrap();
        assert!(doc["created_at"].as_u64().unwrap() > 0);
        let id = doc["id"].as_str().unwrap().to_owned();
        db.rows(
            "MATCH (d:Document) WHERE d.id=$id SET d.created_at=$time",
            vec![
                ("id", lbug::Value::String(id.clone())),
                ("time", lbug::Value::Int64(1000 + index)),
            ],
        )
        .unwrap();
        ids.push(id);
    }
    // Editing the oldest document must not promote it above newer imports.
    let edited = documents::execute(&db, &json!({"action":"save","id":ids[0],"expected_version":1,"title":"Edited oldest","content":"even"})).unwrap();
    assert_eq!(edited["created_at"], 1000);
    let mut seen = vec![];
    for offset in [0, 3, 6] {
        let page =
            documents::execute(&db, &json!({"query":"  ","offset":offset,"limit":3})).unwrap();
        assert_eq!(page["total"], 7);
        assert_eq!(page["truncated"], offset < 6);
        seen.extend(
            page["hits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|doc| doc["id"].as_str().unwrap().to_owned()),
        );
    }
    assert_eq!(seen, ids.into_iter().rev().collect::<Vec<_>>());
    let filtered = documents::execute(&db, &json!({"query":"even","limit":2})).unwrap();
    assert_eq!(filtered["total"], 4);
    assert_eq!(filtered["hits"].as_array().unwrap().len(), 2);
    assert_eq!(filtered["next_offset"], 2);
    let empty = documents::execute(&db, &json!({"offset":100,"limit":20})).unwrap();
    assert_eq!(empty["total"], 7);
    assert_eq!(empty["hits"], json!([]));
}

#[test]
fn instance_library_is_shared_and_other_instances_are_isolated() {
    let root = tempfile::tempdir().unwrap();
    let agent = DocumentStore::new(root.path().into());
    let web = DocumentStore::new(root.path().into());
    let doc=agent.execute(&json!({"action":"save","title":"Article","source":"https://example.com/article","content":"Shared article"})).unwrap();
    assert_eq!(
        web.execute(&json!({"action":"search","query":"Shared"}))
            .unwrap()["hits"][0]["id"],
        doc["id"]
    );
    assert_eq!(
        web.execute(&json!({"action":"read","id":doc["id"]}))
            .unwrap()["chunks"][0]["text"],
        "Shared article"
    );
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        DocumentStore::new(other.path().into())
            .execute(&json!({"action":"search"}))
            .unwrap()["hits"],
        json!([])
    );
    // Old project graphs are neither searched nor migrated.
    let old = Knowledge::at(
        &root
            .path()
            .join("knowledge")
            .join(uuid::Uuid::new_v4().to_string()),
    )
    .unwrap();
    documents::execute(
        &old,
        &json!({"action":"save","title":"Legacy","content":"Not migrated"}),
    )
    .unwrap();
    assert_eq!(
        web.execute(&json!({"action":"search","query":"Legacy"}))
            .unwrap()["hits"],
        json!([])
    );
}

#[test]
fn document_crud_is_versioned_chunked_and_durable() {
    let dir = tempfile::tempdir().unwrap();
    let db = Knowledge::at(dir.path()).unwrap();
    let content = "Unicode 中文 x' MATCH (n) DELETE n ".repeat(120);
    let input = json!({"action":"save","title":"Design","source":"https://example.com/doc","content":content});
    let doc = documents::execute(&db, &input).unwrap();
    let id = doc["id"].clone();
    assert_eq!(
        documents::execute(&db, &input).unwrap()["already_exists"],
        true
    );
    assert_eq!(
        documents::execute(&db, &json!({"action":"read","id":id,"limit":1})).unwrap()["truncated"],
        true
    );
    let read = documents::execute(&db, &json!({"action":"read","id":id,"limit":100})).unwrap();
    assert_eq!(
        read["chunks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["text"].as_str().unwrap())
            .collect::<String>(),
        content
    );
    assert!(
        documents::execute(
            &db,
            &json!({"action":"delete","id":id,"expected_version":0})
        )
        .is_err()
    );
    documents::execute(&db,&json!({"action":"save","id":id,"expected_version":1,"title":"Changed","content":"new text"})).unwrap();
    assert_eq!(
        documents::execute(&db, &json!({"query":"中文"})).unwrap()["hits"],
        json!([])
    );
    drop(db);
    let db = Knowledge::at(dir.path()).unwrap();
    assert_eq!(
        documents::execute(&db, &json!({"action":"read","id":id})).unwrap()["version"],
        2
    );
    documents::execute(
        &db,
        &json!({"action":"delete","id":id,"expected_version":2}),
    )
    .unwrap();
    assert_eq!(
        db.rows("MATCH (c:DocChunk) RETURN count(c)", vec![])
            .unwrap()[0][0]
            .to_string(),
        "0"
    );
}
