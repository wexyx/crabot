use crate::{AppState, control, management::Manager};
use axum::{
    Extension, Json, Router,
    extract::Path,
    http::StatusCode,
    routing::{delete, get, post, put},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
type Reply = Result<Json<Value>, (StatusCode, Json<Value>)>;
fn reply(v: Result<Value, String>) -> Reply {
    v.map(Json).map_err(control::api_error)
}
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/repl", get(index))
        .route(
            "/v1/repl/{project}/chats/{chat}/history",
            get(super::chat_history::history),
        )
        .route(
            "/v1/repl/{project}/chats/{chat}/events",
            get(super::chat_history::events),
        )
        .route("/v1/repl/{project}/groups", get(groups).post(create_group))
        .route("/v1/repl/{project}/groups/{group}", delete(delete_chat))
        .route("/v1/repl/{project}/groups/{group}/name", put(rename_chat))
        .route("/v1/repl/{project}/tools", get(tool_catalog))
        .route("/v1/repl/{project}/groups/{group}/commands", post(command))
        .route(
            "/v1/repl/{project}/groups/{group}/configuration",
            put(configuration),
        )
        .route("/v1/repl/{project}/groups/{group}/messages", post(message))
        .route("/v1/repl/{project}/groups/{group}/sessions", get(topics))
        .route("/v1/repl/{project}/sessions/{session}", get(history))
        .route("/v1/repl/{project}/sessions/{session}/events", get(events))
        .layer(Extension(manager))
}
async fn index(Extension(m): Extension<Arc<Manager>>) -> Reply {
    let mut namespaces = m.core().state().store.list("projects").await;
    let default = m
        .core()
        .state()
        .store
        .get("settings", "cli_project")
        .await
        .unwrap_or_default();
    namespaces.sort_by_key(|row| {
        (
            row["id"] != default["project_id"],
            row["id"].as_str().unwrap_or_default().to_owned(),
        )
    });
    let projects = if let Some(namespace) = namespaces.first() {
        let p = namespace["id"]
            .as_str()
            .unwrap_or_default()
            .parse::<Uuid>()
            .map_err(|e| control::api_error(e.to_string()))?;
        m.workbench(p, "projects.list", json!({}))
            .await
            .map_err(control::api_error)?
    } else {
        json!([])
    };
    // Keep the old index field for existing CLI/API clients; the Web uses the flat catalog.
    reply(Ok(
        json!({"projects":namespaces,"collaboration_projects":projects,"admin_agent":m.status().await}),
    ))
}
async fn create_group(
    Path(p): Path<Uuid>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Reply {
    let name = input["name"].as_str().unwrap_or("").trim();
    if name.len() > 256 {
        return reply(Err("group name maximum 256 bytes".into()));
    }
    reply(
        m.workbench(
            p,
            "projects.create",
            json!({"name":name,"policy":input["policy"],"workspace":input["workspace"]}),
        )
        .await,
    )
}
async fn groups(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.core().control(p, "group.list", json!({})).await)
}
async fn message(
    Path((p, g)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    reply(m.core().group_chat(p,json!({"group_id":g,"content":v["content"],"previous_session_id":v["previous_session_id"]})).await)
}
async fn history(Path((p, s)): Path<(Uuid, Uuid)>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.core().history(p, s).await)
}
async fn topics(
    Path((p, g)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Reply {
    if let Err(error) = m.core().project(p).await {
        return reply(Err(error));
    }
    reply(Ok(json!([{"id":g,"title":g}])))
}
#[derive(serde::Deserialize, Default)]
struct Cursor {
    #[serde(default)]
    after: u64,
}
async fn events(
    Path((p, s)): Path<(Uuid, Uuid)>,
    axum::extract::Query(cursor): axum::extract::Query<Cursor>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<
    axum::response::sse::Sse<
        impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
    >,
    (StatusCode, Json<Value>),
> {
    m.core().project(p).await.map_err(control::api_error)?;
    let stream = futures_util::stream::unfold(
        (m, cursor.after, Vec::<Value>::new()),
        move |(m, mut seq, mut queue)| async move {
            loop {
                if !queue.is_empty() {
                    let row = queue.remove(0);
                    seq = row["seq"].as_u64().unwrap_or(seq);
                    let mut payload = row["payload"].clone();
                    payload["seq"] = json!(seq);
                    return Some((
                        Ok(axum::response::sse::Event::default()
                            .id(seq.to_string())
                            .data(payload.to_string())),
                        (m, seq, queue),
                    ));
                }
                let rows = m.core().history(p, s).await.ok()?;
                queue = rows
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|r| r["seq"].as_u64().unwrap_or(0) > seq)
                    .cloned()
                    .collect();
                if queue.is_empty() {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        },
    );
    Ok(axum::response::sse::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

async fn command(
    Path((p, g)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Reply {
    reply(
        m.core()
            .group_command(p, &g, input["command"].as_str().unwrap_or(""))
            .await,
    )
}
async fn tool_catalog(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.tool_catalog(p).await)
}

async fn configuration(
    Path((p, g)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Reply {
    reply(
        m.workbench(p, "projects.configure", {
            let mut v = input;
            v["id"] = json!(g);
            v
        })
        .await,
    )
}

async fn delete_chat(
    Path((p, g)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(mut input): Json<Value>,
) -> Reply {
    input["id"] = json!(g);
    reply(m.workbench(p, "projects.delete", input).await)
}
async fn rename_chat(
    Path((p, g)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(mut input): Json<Value>,
) -> Reply {
    input["id"] = json!(g);
    reply(m.workbench(p, "projects.rename", input).await)
}
