use crate::management::Manager;
use std::{io::Write, sync::Arc};
use tokio::sync::mpsc;
use uuid::Uuid;

pub(super) enum Update {
    Event(serde_json::Value),
    Notice(String),
    Permissions(Vec<super::permission_dialog::Permission>),
}
pub(super) async fn subscribe(
    manager: Arc<Manager>,
    project: Uuid,
    id: Uuid,
    group: Option<String>,
    replay: bool,
) -> (mpsc::UnboundedReceiver<Update>, tokio::task::JoinHandle<()>) {
    let business = group.is_some();
    let mut events = manager.subscribe();
    let (tx, rx) = mpsc::unbounded_channel();
    let baseline = if replay {
        0
    } else {
        let chat = group
            .as_ref()
            .map(|g| format!("group:{g}"))
            .unwrap_or_else(|| "admin".into());
        crate::storage::knowledge::for_project(project)
            .and_then(|knowledge| crate::storage::knowledge::latest_seq(&knowledge, &chat).ok())
            .flatten()
            .unwrap_or(0)
    };
    let task = tokio::spawn(async move {
        let mut seen = baseline;
        let mut shown = std::collections::HashSet::new();
        let mut reload = true;
        let mut approvals = tokio::time::interval(std::time::Duration::from_millis(500));
        let mut group_poll = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            if reload {
                let result = if business {
                    let chat = format!("group:{}", group.as_deref().unwrap());
                    let history =
                        crate::core::indexed_history::IndexedHistory::new(project, chat, None);
                    history
                        .rows(None, seen, u64::MAX, 10000)
                        .await
                        .map(|(rows, _)| serde_json::Value::Array(rows))
                } else {
                    manager
                        .history(project, id)
                        .await
                        .map(|r| r["events"].clone())
                };
                match result {
                    Ok(rows) => {
                        for row in rows.as_array().into_iter().flatten() {
                            let seq = row["seq"].as_u64().unwrap_or(0);
                            if seq > seen {
                                seen = seq;
                                if tx.send(Update::Event(row.clone())).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Update::Notice(e));
                        return;
                    }
                }
                reload = false;
            }
            tokio::select! {
                _=tx.closed()=>return,
                item=events.recv(),if !business=>match item {
                    Ok(item) if item.session==id=>{
                        let seq=item.event["seq"].as_u64().unwrap_or(0);
                        if seq>seen {seen=seq;if tx.send(Update::Event(item.event)).is_err(){return;}}
                    },
                    Ok(_)=>{},
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>reload=true,
                    Err(_)=>return,
                },
                _=group_poll.tick(),if business=>reload=true,
                _=approvals.tick()=>{
                    let mut permissions=vec![];
                    if let Ok(pending)=manager.core().approvals(project).await {
                        for item in pending.as_array().into_iter().flatten() {
                            if let Some(permission)=super::permission_dialog::Permission::management(project,item){permissions.push(permission);}
                            if shown.insert(item["id"].to_string()) {let _=tx.send(Update::Notice(super::presentation::Presentation::approval(item)));}
                        }
                    }
                    for item in agent_runtime::workspace::pending() {
                        permissions.push(super::permission_dialog::Permission{id:item.id,project,workspace:true,conversation:item.command.is_some()&&item.conversation_id.is_some(),title:if item.command.is_some(){"执行命令 · 风险操作".into()}else{format!("目录外访问 · {}",item.operation)},detail:format!("工作目录：{}\n{}\n{}\n请求 ID：{}",item.workdir,item.operation,item.command.as_deref().unwrap_or(&item.path),item.id)});
                        if shown.insert(item.id.to_string()) {
                            let _=tx.send(Update::Notice(format!("目录访问：{} · {}\n/allow-path {} 或 /deny-path {}",item.operation,item.command.as_deref().unwrap_or(&item.path),item.id,item.id)));
                        }
                    }
                    permissions.sort_by_key(|p|p.id);
                    let _=tx.send(Update::Permissions(permissions));
                }
            }
        }
    });
    (rx, task)
}
pub(super) async fn watch(
    manager: Arc<Manager>,
    project: Uuid,
    id: Uuid,
    group: Option<String>,
    replay: bool,
) -> tokio::task::JoinHandle<()> {
    let business = group.is_some();
    let (mut rx, _task) = subscribe(manager, project, id, group, replay).await;
    tokio::spawn(async move {
        let mut presentation = super::presentation::Presentation::default();
        while let Some(update) = rx.recv().await {
            match update {
                Update::Permissions(_) => {}
                Update::Notice(text) => println!("\n{text}"),
                Update::Event(row) => {
                    let event = row.get("payload").unwrap_or(&row);
                    if replay && matches!(event["type"].as_str(), Some("user" | "message.created"))
                    {
                        println!("\n你：{}", event["content"].as_str().unwrap_or_default());
                    }
                    print!("{}", presentation.render(&row));
                    let event = row.get("payload").unwrap_or(&row);
                    if matches!(
                        event["type"].as_str(),
                        Some(
                            "completed"
                                | "agent.done"
                                | "failed"
                                | "agent.error"
                                | "task.interrupted"
                        )
                    ) {
                        print!("{}", if business { "group> " } else { "admin> " });
                    }
                }
            }
            let _ = std::io::stdout().flush();
        }
    })
}
