use super::SkillCatalog;
use crate::attachments::AttachmentStore;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct ImageState {
    root: PathBuf,
    token: String,
    store: AttachmentStore,
    active: Arc<AtomicBool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageInput {
    path: String,
}

/// Authenticated local service for an enabled Skill, scoped to one approved shell
/// execution. It is not registered as a model tool or exposed on the Web server.
pub(crate) struct SkillBridge {
    environment: BTreeMap<String, String>,
    task: tokio::task::JoinHandle<()>,
    active: Arc<AtomicBool>,
}
impl SkillBridge {
    pub(crate) async fn start(
        root: PathBuf,
        catalog: &SkillCatalog,
    ) -> Result<Option<Self>, String> {
        Self::bind(root, catalog, AttachmentStore::default()).await
    }
    async fn bind(
        root: PathBuf,
        catalog: &SkillCatalog,
        store: AttachmentStore,
    ) -> Result<Option<Self>, String> {
        if catalog.get("image-view").is_err() {
            return Ok(None);
        }
        let token = uuid::Uuid::new_v4().to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        let endpoint = format!(
            "http://{}/image",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let active = Arc::new(AtomicBool::new(true));
        let state = Arc::new(ImageState {
            root,
            token: token.clone(),
            store,
            active: active.clone(),
        });
        let router = Router::new()
            .route("/image", post(publish))
            .layer(DefaultBodyLimit::max(8192))
            .with_state(state);
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Some(Self {
            environment: BTreeMap::from([
                ("CRABOT_SKILL_ENDPOINT".into(), endpoint),
                ("CRABOT_SKILL_TOKEN".into(), token),
            ]),
            task,
            active,
        }))
    }
    pub(crate) fn environment(&self) -> BTreeMap<String, String> {
        self.environment.clone()
    }
}
impl Drop for SkillBridge {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        self.task.abort();
    }
}
async fn publish(
    State(state): State<Arc<ImageState>>,
    headers: HeaderMap,
    Json(input): Json<ImageInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !state.active.load(Ordering::SeqCst)
        || headers.get("authorization").and_then(|v| v.to_str().ok())
            != Some(format!("Bearer {}", state.token).as_str())
    {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unauthorized"})),
        ));
    }
    super::image_publish::publish(&state.root, &input.path, &state.store)
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(json!({"error":error}))))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn image_skill_bridge_is_scoped_authenticated_and_runs_the_packaged_script() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let store = AttachmentStore::new(root.join("attachments"));
        assert!(
            SkillBridge::bind(
                root.clone(),
                &SkillCatalog::default(),
                AttachmentStore::new(root.join("attachments"))
            )
            .await
            .unwrap()
            .is_none()
        );
        let catalog = SkillCatalog::new(vec![
            super::super::SkillDefinition::new(
                "image-view".into(),
                "show images".into(),
                BTreeMap::from([("SKILL.md".into(), "instructions".into())]),
                true,
                false,
            )
            .unwrap(),
        ])
        .unwrap();
        let bridge = SkillBridge::bind(root.clone(), &catalog, store)
            .await
            .unwrap()
            .unwrap();
        let env = bridge.environment();
        let endpoint = &env["CRABOT_SKILL_ENDPOINT"];
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let unauthorized = client
            .post(endpoint)
            .json(&json!({"path":"x"}))
            .send()
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), 401);
        for path in ["http://169.254.169.254/metadata", "../outside.png"] {
            let response = client
                .post(endpoint)
                .bearer_auth(&env["CRABOT_SKILL_TOKEN"])
                .json(&json!({"path":path}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 400);
        }
        let image = crate::workspace::temporary_dir(&root)
            .unwrap()
            .join("image.png");
        std::fs::write(&image, b"\x89PNG\r\n\x1a\nfixture").unwrap();
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/system/business/image-view/show.py");
        let output = tokio::process::Command::new("python3")
            .arg(script)
            .arg(&image)
            .envs(&env)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            value["reference"]
                .as_str()
                .unwrap()
                .contains("/v1/attachments/")
        );
        drop(bridge);
        tokio::task::yield_now().await;
        let request = client
            .post(endpoint)
            .bearer_auth(&env["CRABOT_SKILL_TOKEN"])
            .json(&json!({"path":image}))
            .send()
            .await;
        assert!(request.is_err() || request.unwrap().status() == 401);
    }
}
