use agent_runtime::context::Guidance;
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
pub(crate) struct Request {
    pub content: String,
    pub reply: oneshot::Sender<Result<(), String>>,
}
struct Entry {
    sender: mpsc::Sender<Request>,
    inbox: Arc<Guidance>,
}
static ACTIVE: LazyLock<Mutex<HashMap<(Uuid, String), Entry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub(crate) struct Mailbox {
    project: Uuid,
    chat: String,
    inbox: Arc<Guidance>,
    receiver: mpsc::Receiver<Request>,
}
impl Mailbox {
    pub(crate) fn new(project: Uuid, chat: String) -> Self {
        let (sender, receiver) = mpsc::channel(16);
        let inbox = Arc::new(Guidance::default());
        ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).insert(
            (project, chat.clone()),
            Entry {
                sender,
                inbox: inbox.clone(),
            },
        );
        Self {
            project,
            chat,
            inbox,
            receiver,
        }
    }
    pub(crate) fn inbox(&self) -> Arc<Guidance> {
        self.inbox.clone()
    }
    pub(crate) async fn recv(&mut self) -> Option<Request> {
        self.receiver.recv().await
    }
    pub(crate) fn close(&mut self) {
        self.receiver.close();
        while let Ok(r) = self.receiver.try_recv() {
            let _ = r
                .reply
                .send(Err("任务已结束，引导未接收，请重新发送".into()));
        }
    }
}
impl Drop for Mailbox {
    fn drop(&mut self) {
        self.close();
        let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        let key = (self.project, self.chat.clone());
        if active
            .get(&key)
            .is_some_and(|entry| Arc::ptr_eq(&entry.inbox, &self.inbox))
        {
            active.remove(&key);
        }
    }
}
pub(crate) fn inbox(project: Uuid, chat: &str) -> Arc<Guidance> {
    ACTIVE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(project, chat.into()))
        .map(|e| e.inbox.clone())
        .unwrap_or_default()
}
pub(crate) async fn send(project: Uuid, chat: &str, content: String) -> Result<(), String> {
    let sender = ACTIVE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(project, chat.into()))
        .map(|e| e.sender.clone())
        .ok_or("任务正在结束，请稍后重新发送")?;
    let (reply, receive) = oneshot::channel();
    sender
        .try_send(Request { content, reply })
        .map_err(|_| "引导队列已满或任务已结束，请稍后重试")?;
    receive
        .await
        .map_err(|_| "任务已结束，引导未接收，请重新发送".to_string())?
}
