use axum::{Json, extract::Query, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Deserialize)]
pub(crate) struct DirectoryQuery {
    path: Option<PathBuf>,
}

// Human-facing directory metadata only. Never exposed to A2A or model tools.
pub(crate) async fn list(
    Query(query): Query<DirectoryQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    browse(query.path).await.map(Json).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": error.to_string()})),
        )
    })
}

async fn browse(path: Option<PathBuf>) -> std::io::Result<Value> {
    let path = path.unwrap_or_else(agent_runtime::paths::workdir);
    let path = tokio::fs::canonicalize(path).await?;
    let mut entries = tokio::fs::read_dir(&path).await?;
    let mut directories = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            directories
                .push(json!({"name":entry.file_name().to_string_lossy(),"path":entry.path()}));
        }
    }
    directories.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!({"path":path,"parent":path.parent(),"directories":directories}))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn only_lists_directories() {
        let root = std::env::temp_dir().join(format!("crabot-picker-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join("child")).await.unwrap();
        tokio::fs::write(root.join("secret.txt"), "not exposed")
            .await
            .unwrap();
        let result = super::browse(Some(root.clone())).await.unwrap();
        assert_eq!(result["directories"].as_array().unwrap().len(), 1);
        assert_eq!(result["directories"][0]["name"], "child");
        assert!(super::browse(Some(root.join("secret.txt"))).await.is_err());
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
