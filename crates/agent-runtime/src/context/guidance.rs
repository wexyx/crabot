use std::{
    future::Future,
    sync::{Arc, Mutex},
};
#[derive(Default)]
pub struct Guidance {
    pending: Mutex<Vec<String>>,
}
tokio::task_local! { static ACTIVE: Arc<Guidance>; }
impl Guidance {
    pub fn has_room(&self) -> bool {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len() < 16
    }
    pub fn push(&self, text: String) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(text);
    }
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()))
    }
    pub async fn scope<F: Future>(inbox: Arc<Self>, work: F) -> F::Output {
        ACTIVE.scope(inbox, work).await
    }
    pub(crate) fn pending() -> bool {
        ACTIVE
            .try_with(|q| {
                !q.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_empty()
            })
            .unwrap_or(false)
    }
    pub(crate) fn drain() -> Vec<String> {
        ACTIVE.try_with(|q| q.take()).unwrap_or_default()
    }
}
