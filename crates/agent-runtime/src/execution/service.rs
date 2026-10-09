use super::profile::Profile;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{OnceCell, Semaphore, watch};
static SERVICE: OnceCell<Arc<ExecutionService>> = OnceCell::const_new();

pub(super) struct ExecutionService {
    profiles: HashMap<String, Profile>,
    slots: Arc<Semaphore>,
    stopping: watch::Sender<bool>,
}
impl ExecutionService {
    pub(super) fn new(profiles: Vec<Profile>) -> Result<Self, String> {
        if profiles.is_empty() || profiles.len() > 16 {
            return Err("execution requires 1..16 profiles".into());
        }
        let mut selected = HashMap::new();
        for profile in profiles {
            profile.validate()?;
            if selected.insert(profile.id.clone(), profile).is_some() {
                return Err("duplicate execution profile".into());
            }
        }
        let (stopping, _) = watch::channel(false);
        Ok(Self {
            profiles: selected,
            slots: Arc::new(Semaphore::new(4)),
            stopping,
        })
    }
    pub(super) async fn supervise<F, Fut>(
        self: &Arc<Self>,
        profile: String,
        execute: F,
    ) -> Result<Value, String>
    where
        F: FnOnce(Profile) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Value, String>> + Send + 'static,
    {
        if *self.stopping.borrow() {
            return Err("execution is shutting down".into());
        }
        let profile = self
            .profiles
            .get(&profile)
            .cloned()
            .ok_or("unknown execution profile")?;
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "execution closed")?;
        let mut stopping = self.stopping.subscribe();
        if *stopping.borrow() {
            return Err("execution is shutting down".into());
        }
        let (mut tx, rx) = tokio::sync::oneshot::channel();
        let mode = crate::permissions::PermissionMode::current();
        tokio::spawn(mode.scope(async move {
            let _permit = permit;
            // Dropping the execution future kills its process group and deletes staging files.
            // The supervisor retains the permit until those RAII guards have run.
            let outcome = tokio::select! {
                _=tx.closed()=>Err("execution caller cancelled".into()),
                _=stopping.changed()=>Err("execution shutdown".into()),
                result=tokio::time::timeout(Duration::from_secs(profile.timeout_seconds),execute(profile.clone()))=>result.unwrap_or_else(|_|Err("execution timed out".into())),
            };
            let _ = tx.send(outcome);
        }));
        rx.await.map_err(|_| "execution supervisor stopped")?
    }
    pub(super) async fn shutdown(&self) {
        self.stopping.send_replace(true);
        let _ = tokio::time::timeout(Duration::from_secs(5), self.slots.acquire_many(4)).await;
        self.slots.close();
    }
}
async fn service() -> Result<&'static Arc<ExecutionService>, String> {
    SERVICE
        .get_or_try_init(|| async {
            let defaults = r#"[{"id":"default","network":"host","timeout_seconds":120}]"#;
            let profiles = serde_json::from_str(
                &std::env::var("CRABOT_EXECUTION_PROFILES_JSON")
                    .or_else(|_| std::env::var("CRABOT_SANDBOX_PROFILES_JSON"))
                    .unwrap_or_else(|_| defaults.into()),
            )
            .map_err(|e| e.to_string())?;
            Ok(Arc::new(ExecutionService::new(profiles)?))
        })
        .await
}
pub async fn initialize() -> Result<(), String> {
    service().await?;
    Ok(())
}
pub async fn shutdown() {
    if let Some(service) = SERVICE.get() {
        service.shutdown().await;
    }
}

pub(crate) async fn execute_command(
    root: std::path::PathBuf,
    command: String,
    profile: String,
    environment: std::collections::BTreeMap<String, String>,
) -> Result<Value, String> {
    service()
        .await?
        .supervise(profile, move |profile| {
            super::native::shell::execute(root, command, profile, environment)
        })
        .await
}
