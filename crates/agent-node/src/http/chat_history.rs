use crate::{control, management::Manager};
use axum::{
    Extension, Json,
    extract::{Path, Query},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc};
use uuid::Uuid;
#[derive(Deserialize)]
pub(crate) struct Page {
    #[serde(default)]
    after: u64,
    #[serde(default)]
    before: Option<u64>,
    #[serde(default)]
    limit: Option<usize>,
}
async fn key(m: &Manager, p: Uuid, chat: &str) -> Result<String, String> {
    m.core().project(p).await?;
    if chat == "admin" {
        return Ok("admin".into());
    }
    m.core().state().policy_store.get(p, "group", chat).await?;
    Ok(format!("group:{chat}"))
}
pub(crate) async fn history(
    Path((p, chat)): Path<(Uuid, String)>,
    Query(page): Query<Page>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let key = key(&m, p, &chat).await.map_err(control::api_error)?;
    // Read from the knowledge index, not from log files: the index is the only
    // chat-record store and supports sequential reads with summary awareness.
    let history = crate::core::indexed_history::IndexedHistory::new(p, key, None);
    let (rows, truncated) = history
        .rows(
            None,
            page.after,
            page.before.unwrap_or(u64::MAX),
            page.limit.unwrap_or(300).clamp(1, 1000),
        )
        .await
        .map_err(control::api_error)?;
    let events = rows.into_iter().map(flatten).collect::<Vec<_>>();
    let runs = m.core().state().store.list("runs").await;
    let active = runs
        .iter()
        .find(|r| r["project_id"] == json!(p) && r["group_id"] == chat && r["status"] == "running");
    Ok(Json(
        json!({"events":events,"active_run":active.map(|r|r["id"].clone()),"has_more":truncated}),
    ))
}
fn flatten(mut row: Value) -> Value {
    if row.get("payload").is_some() {
        let mut event = row["payload"].take();
        event["seq"] = row["seq"].clone();
        event["logged_at"] = row["logged_at"].clone();
        event
    } else {
        row
    }
}
pub(crate) async fn events(
    Path((p, chat)): Path<(Uuid, String)>,
    Query(page): Query<Page>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let key = key(&m, p, &chat).await.map_err(control::api_error)?;
    let stream = futures_util::stream::unfold(
        (p, key, page.after, std::collections::VecDeque::new(), false),
        move |(project, key, mut seq, mut queue, failed)| async move {
            if failed {
                return None;
            }
            loop {
                if let Some(row) = queue.pop_front() {
                    let row: Value = row;
                    seq = row["seq"].as_u64().unwrap_or(seq);
                    return Some((
                        Ok(Event::default()
                            .id(seq.to_string())
                            .data(flatten(row).to_string())),
                        (project, key, seq, queue, false),
                    ));
                }
                // The live poll reads the index, which every durable write lands in
                // synchronously, so nothing newer can hide in an unflushed buffer.
                let history =
                    crate::core::indexed_history::IndexedHistory::new(project, key.clone(), None);
                match history.rows(None, seq, u64::MAX, 1000).await {
                    Ok((rows, _)) => queue = rows.into(),
                    Err(error) => {
                        return Some((
                            Ok(Event::default()
                                .data(json!({"type":"stream_error","message":error}).to_string())),
                            (project, key, seq, queue, true),
                        ));
                    }
                }
                if queue.is_empty() {
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
            }
        },
    );
    Ok(Sse::new(
        futures_util::stream::once(async { Ok(Event::default().comment("connected")) })
            .chain(stream),
    )
    .keep_alive(KeepAlive::default()))
}
