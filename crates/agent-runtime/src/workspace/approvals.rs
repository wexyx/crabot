use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use uuid::Uuid;
tokio::task_local! { static CORRELATION: Uuid; static CONVERSATION: String; }
static CONVERSATION_GRANTS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
fn grants() -> &'static Mutex<BTreeSet<String>> {
    CONVERSATION_GRANTS.get_or_init(Default::default)
}
pub async fn with_conversation_approval<F: std::future::Future>(
    id: String,
    future: F,
) -> F::Output {
    CONVERSATION.scope(id, future).await
}
pub fn revoke_conversation_approval(id: &str) {
    grants().lock().unwrap().remove(id);
}
pub fn allow_conversation(id: Uuid) -> Result<(), String> {
    let mut entries = queue().lock().unwrap();
    let entry = entries
        .get(&id)
        .ok_or("approval expired or already decided")?;
    if entry.created.elapsed() >= Duration::from_secs(120) || entry.sender.is_closed() {
        return Err("approval expired".into());
    }
    if entry.view.command.is_none() {
        return Err("目录外访问仍需逐次确认".into());
    }
    let conversation = entry
        .view
        .conversation_id
        .clone()
        .ok_or("此请求没有可授权的项目对话")?;
    let mut allowed = grants().lock().unwrap();
    if allowed.len() >= 4096 && !allowed.contains(&conversation) {
        return Err("too many conversation grants".into());
    }
    allowed.insert(conversation.clone());
    let ids = entries
        .iter()
        .filter(|(_, p)| {
            p.view.conversation_id.as_ref() == Some(&conversation) && p.view.command.is_some()
        })
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    for id in ids {
        if let Some(p) = entries.remove(&id) {
            let _ = p.sender.send(true);
        }
    }
    Ok(())
}
pub async fn with_approval_context<F: std::future::Future>(id: Uuid, future: F) -> F::Output {
    CORRELATION.scope(id, future).await
}

#[derive(Clone, Serialize)]
pub struct ApprovalRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub id: Uuid,
    pub workdir: String,
    pub path: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}
struct Pending {
    view: ApprovalRequest,
    sender: oneshot::Sender<bool>,
    created: Instant,
}
static PENDING: OnceLock<Mutex<BTreeMap<Uuid, Pending>>> = OnceLock::new();
fn queue() -> &'static Mutex<BTreeMap<Uuid, Pending>> {
    PENDING.get_or_init(Default::default)
}
struct Ticket(Uuid);
impl Drop for Ticket {
    fn drop(&mut self) {
        queue().lock().unwrap().remove(&self.0);
    }
}
pub fn pending() -> Vec<ApprovalRequest> {
    let mut queue = queue().lock().unwrap();
    queue.retain(|_, p| !p.sender.is_closed() && p.created.elapsed() < Duration::from_secs(120));
    queue.values().map(|p| p.view.clone()).collect()
}
pub fn decide(id: Uuid, allow: bool) -> Result<(), String> {
    let entry = queue()
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("approval expired or already decided")?;
    if entry.created.elapsed() >= Duration::from_secs(120) {
        return Err("approval expired".into());
    }
    entry
        .sender
        .send(allow)
        .map_err(|_| "approval caller stopped".into())
}
pub(super) async fn request(
    workdir: &std::path::Path,
    path: &std::path::Path,
    operation: &str,
) -> Result<(), String> {
    enqueue(workdir, path, operation, None).await
}
pub(crate) async fn confirm_command(
    workdir: &std::path::Path,
    command: &str,
    profile: &str,
) -> Result<(), String> {
    if crate::permissions::PermissionMode::current().approves_command(command) {
        return Ok(());
    }
    enqueue(workdir,workdir,&format!("执行 Shell 命令（profile: {profile}）。拥有当前系统用户的文件和网络权限，可能修改/删除工作目录外的文件；工作目录不是隔离边界。"),Some(command.into())).await
}
async fn enqueue(
    workdir: &std::path::Path,
    path: &std::path::Path,
    operation: &str,
    command: Option<String>,
) -> Result<(), String> {
    let id = Uuid::new_v4();
    let (sender, receiver) = oneshot::channel();
    {
        let mut entries = queue().lock().unwrap();
        let conversation_id = CONVERSATION.try_with(Clone::clone).ok();
        if command.is_some()
            && conversation_id
                .as_ref()
                .is_some_and(|id| grants().lock().unwrap().contains(id))
        {
            return Ok(());
        }
        if entries.len() >= 64 {
            return Err("too many pending workspace approvals".into());
        }
        entries.insert(
            id,
            Pending {
                view: ApprovalRequest {
                    conversation_id,
                    correlation_id: CORRELATION.try_with(|id| *id).ok(),
                    id,
                    workdir: workdir.display().to_string(),
                    path: path.display().to_string(),
                    operation: operation.into(),
                    command,
                },
                sender,
                created: Instant::now(),
            },
        );
    }
    let _ticket = Ticket(id);
    match tokio::time::timeout(Duration::from_secs(120), receiver).await {
        Ok(Ok(true)) => Ok(()),
        Ok(Ok(false)) => Err("human permission rejected".into()),
        _ => Err("human permission expired or cancelled".into()),
    }
}
