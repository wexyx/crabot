use crate::management::Manager;
use serde_json::json;
pub(super) async fn command(m: &Manager, args: &str) -> Result<String, String> {
    let (op, tail) = args.trim().split_once(' ').unwrap_or((args.trim(), ""));
    let tail = tail.trim();
    let value = match op {
        "" | "search" => m.documents(json!({"action":"search","query":tail})).await?,
        "read" => {
            m.documents(json!({"action":"read","id":tail,"limit":100}))
                .await?
        }
        "import" => {
            if tail.starts_with('{') {
                let mut input: serde_json::Value =
                    serde_json::from_str(tail).map_err(|e| e.to_string())?;
                input["action"] = json!("import");
                m.documents(input).await?
            } else if tail.starts_with("http://") || tail.starts_with("https://") {
                m.documents(json!({"action":"import","url":tail})).await?
            } else {
                let path = std::path::Path::new(tail);
                let info = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
                if !info.is_file() || info.len() > 20 * 1024 * 1024 {
                    return Err("请选择不超过 20 MiB 的文档".into());
                }
                let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
                m.document_service()
                    .upload(
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        bytes,
                    )
                    .await?
            }
        }
        "save" | "delete" => {
            let mut value:serde_json::Value=serde_json::from_str(tail).map_err(|_|"/docs save {\"title\":\"标题\",\"content\":\"正文\"}；修改/删除需 id 和 expected_version")?;
            value["action"] = json!(op);
            m.documents(value).await?
        }
        _ => {
            return Err(
                "/docs [search 关键词 | read ID | import URL或文件路径 | save JSON | delete JSON]"
                    .into(),
            );
        }
    };
    Ok(serde_json::to_string_pretty(&value)
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect())
}
