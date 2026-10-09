use std::future::Future;

tokio::task_local! { static CURRENT: SessionScope; }

/// Host-provided identity, never taken from model tool arguments.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionScope {
    project: String,
    chat: String,
    agent: String,
    data: std::path::PathBuf,
}
impl SessionScope {
    pub fn new(project: String, chat: String, agent: String) -> Self {
        Self {
            project,
            chat,
            agent,
            data: crate::paths::data_dir(),
        }
    }
    pub(crate) fn with_data(mut self, data: std::path::PathBuf) -> Self {
        self.data = data;
        self
    }
    pub(crate) fn current() -> Option<Self> {
        CURRENT.try_with(Clone::clone).ok()
    }
    pub(super) fn project(&self) -> &str {
        &self.project
    }
    pub(super) fn chat(&self) -> &str {
        &self.chat
    }
    pub(super) fn agent(&self) -> &str {
        &self.agent
    }
    pub(crate) fn home(&self) -> std::path::PathBuf {
        fn key(s: &str) -> String {
            s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
        }
        self.data
            .join("runtime/shell")
            .join(key(&self.project))
            .join(key(&self.agent))
            .join("home")
    }
    /// Completed model turns keep processes. Failed/cancelled turns clean their scope.
    pub async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let mut guard = CancelGuard(Some(self.clone()));
        let result = CURRENT.scope(self.clone(), future).await;
        if result.is_ok() {
            guard.0 = None;
        }
        result
    }
}
struct CancelGuard(Option<SessionScope>);
impl Drop for CancelGuard {
    fn drop(&mut self) {
        if let Some(scope) = self.0.take() {
            super::ProcessSessions::global().stop_owner(&scope);
        }
    }
}
