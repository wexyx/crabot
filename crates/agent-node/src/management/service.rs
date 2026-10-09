use super::{context::Context, session, skills};
use crate::core::Core;
use agent_runtime::{
    RuntimeFactory,
    config::RuntimeConfig,
    skills::ExecutionPolicy,
    tools::{ToolContext, ToolFactory, ToolRegistry},
};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;

pub(crate) struct Manager {
    test_runs: super::test_runs::TestRuns,
    core: Core,
    journal: super::journal::Journal,
    provider: Mutex<Option<RuntimeConfig>>,
    settings: Mutex<Option<crate::configuration::Settings>>,
    active: Mutex<HashMap<Uuid, watch::Sender<bool>>>,
    web: Arc<crate::web_server::WebServer>,
}
impl Manager {
    pub(crate) fn new(core: Core) -> Arc<Self> {
        Arc::new(Self {
            test_runs: Default::default(),
            core,
            journal: Default::default(),
            provider: Mutex::new(None),
            settings: Mutex::new(None),
            active: Mutex::new(HashMap::new()),
            web: Arc::new(crate::web_server::WebServer::default()),
        })
    }
    pub(super) fn active_sessions(&self) -> &Mutex<HashMap<Uuid, watch::Sender<bool>>> {
        &self.active
    }
    pub(crate) fn core(&self) -> &Core {
        &self.core
    }
    pub(super) fn test_runs(&self) -> &super::test_runs::TestRuns {
        &self.test_runs
    }
    pub(crate) fn web(&self) -> &Arc<crate::web_server::WebServer> {
        &self.web
    }
    #[cfg(test)]
    pub(crate) async fn start(&self, provider: &str) -> Result<Value, String> {
        self.configure(RuntimeConfig::from_env(provider.parse()?)?)
            .await?;
        Ok(json!({"status":"started","provider":provider,"scope":"management"}))
    }
    pub(crate) async fn configure(&self, config: RuntimeConfig) -> Result<(), String> {
        let active = self.active.lock().await;
        if !active.is_empty() {
            return Err("默认 Agent 有正在执行的任务，请等待完成或中断后再修改配置".into());
        }
        self.core.default_agent(config.clone(), || Ok(())).await?;
        *self.provider.lock().await = Some(config);
        Ok(())
    }
    pub(crate) async fn configuration(&self) -> Result<crate::configuration::Settings, String> {
        match self.settings.lock().await.clone() {
            Some(settings) => Ok(settings),
            None => crate::configuration::Settings::load(),
        }
    }
    pub(crate) async fn reconfigure(
        &self,
        settings: crate::configuration::Settings,
    ) -> Result<(), String> {
        let config = settings.runtime()?;
        let active = self.active.lock().await;
        if !active.is_empty() {
            return Err("默认 Agent 有正在执行的任务，请等待完成或中断后再修改配置".into());
        }
        self.core
            .default_agent(config.clone(), || settings.save())
            .await?;
        *self.settings.lock().await = Some(settings);
        *self.provider.lock().await = Some(config);
        Ok(())
    }
    pub(crate) async fn ensure_idle(&self) -> Result<(), String> {
        if self.active.lock().await.is_empty() {
            Ok(())
        } else {
            Err("默认 Agent 有正在执行的任务，请等待完成或中断后再修改配置".into())
        }
    }
    pub(crate) async fn stop(&self) -> Value {
        self.test_runs.stop().await;
        *self.provider.lock().await = None;
        for sender in self.active.lock().await.values() {
            let _ = sender.send(true);
        }
        for _ in 0..100 {
            if self.active.lock().await.is_empty() {
                return json!({"status":"stopped"});
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        json!({"status":"stopping"})
    }
    pub(crate) async fn status(&self) -> Value {
        // Own each snapshot before awaiting another lock. json! temporaries otherwise
        // keep the provider guard alive while waiting for active (opposite to message).
        let provider = self
            .provider
            .lock()
            .await
            .as_ref()
            .map(|c| c.kind().as_str().to_owned());
        let active_sessions = self.active.lock().await.keys().copied().collect::<Vec<_>>();
        let web = self.web.address().await;
        json!({"provider":provider,"active_sessions":active_sessions,"web":web})
    }
    pub(crate) async fn registry(self: &Arc<Self>, project: Uuid) -> Result<ToolRegistry, String> {
        let mut registry = self.base_registry(project).await?;
        let policy =
            crate::core::tool_policy::load(self.core().state(), project, "management", "admin")
                .await?;
        for name in policy.disabled() {
            registry.unregister(name);
        }
        Ok(registry)
    }
    pub(crate) async fn base_registry(
        self: &Arc<Self>,
        project: Uuid,
    ) -> Result<ToolRegistry, String> {
        self.core.project(project).await?;
        let context = ToolContext::new(
            None,
            skills::catalog(&self.core, project).await?,
            ExecutionPolicy::new("offline".into())?,
        )?
        .with_extension(Arc::new(Context::new(
            self.core.clone(),
            project,
            Arc::downgrade(self),
        )));
        ToolFactory::create_scoped(context, "management")
    }
    pub(crate) async fn create_session(&self, project: Uuid) -> Result<Value, String> {
        self.core.project(project).await?;
        // Exactly one management chat per project; executions append to it.
        let id = project;
        if let Some(row) = self
            .core
            .state()
            .store
            .get("management_sessions", &id.to_string())
            .await
        {
            return Ok(row);
        }
        let row = json!({"id":id,"project_id":project,"status":"idle","updated_at":crate::storage::now()});
        self.core
            .state()
            .store
            .transaction(|d| {
                if d.get("management_sessions", &id.to_string()).is_none() {
                    d.set("management_sessions", &id.to_string(), row.clone());
                }
                Ok(())
            })
            .await?;
        Ok(row)
    }
    pub(crate) async fn sessions(&self, project: Uuid) -> Result<Value, String> {
        let row = self.create_session(project).await?;
        let history =
            crate::core::indexed_history::IndexedHistory::new(project, "admin".into(), None);
        let (events, _) = history.rows(None, 0, u64::MAX, 1).await?;
        let preview = events
            .last()
            .and_then(|e| e["text"].as_str().or(e["content"].as_str()))
            .unwrap_or("");
        Ok(
            json!([{"id":row["id"],"status":row["status"],"title":"管理","preview":preview,"updated_at":row["updated_at"]}]),
        )
    }
    pub(crate) async fn history(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        self.core.project(project).await?;
        let mut row = self
            .core
            .state()
            .store
            .get("management_sessions", &id.to_string())
            .await
            .ok_or("unknown management session")?;
        if row["project_id"] != json!(project) {
            return Err("forbidden: session belongs to another project".into());
        }
        let live = self.journal.history(id);
        // Read from the knowledge index for sequential history (display view).
        let history =
            crate::core::indexed_history::IndexedHistory::new(project, "admin".into(), None);
        let (mut events, _) = history.rows(None, 0, u64::MAX, 10000).await?;
        if let Some(live) = live {
            row["status"] = live["status"].clone();
            row["persistence_failed"] = live["persistence_failed"].clone();
            let last = events.last().and_then(|e| e["seq"].as_u64()).unwrap_or(0);
            events.extend(
                live["events"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e["seq"].as_u64().unwrap_or(0) > last)
                    .cloned(),
            );
        }
        row["events"] = json!(events);
        Ok(row)
    }
    pub(crate) async fn interrupt(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        self.history(project, id).await?;
        agent_runtime::execution::ProcessSessions::global()
            .stop_chat(&project.to_string(), "admin");
        let active = self.active.lock().await;
        if let Some(sender) = active.get(&id) {
            let _ = sender.send(true);
        }
        Ok(json!({"status":"interrupt_requested"}))
    }
    pub(crate) async fn message(
        self: &Arc<Self>,
        project: Uuid,
        id: Uuid,
        content: String,
    ) -> Result<Value, String> {
        if content.trim().is_empty() || content.len() > 65536 {
            return Err("message requires 1..65536 bytes".into());
        }
        let mut active = self.active.lock().await;
        let provider = self
            .provider
            .lock()
            .await
            .clone()
            .ok_or("management agent not started")?;
        if active.contains_key(&id) {
            self.history(project, id).await?;
            crate::core::steering::send(project, "admin", content).await?;
            return Ok(json!({"session_id":id,"status":"steering_accepted"}));
        }
        if active.len() >= 8 {
            return Err("too many active management sessions".into());
        }
        let mut history = self.history(project, id).await?;
        if history["persistence_failed"] == true {
            return Err("previous persistence failed; repair storage and restart before continuing this session".into());
        }
        let source = Arc::new(crate::core::indexed_history::IndexedHistory::new(
            project,
            "admin".into(),
            Some("default".into()),
        ));
        let context_rows = source.context_rows(None).await?;
        let records = super::history::context(&json!(context_rows));

        let registry = self.registry(project).await?;
        let catalog = skills::catalog(&self.core, project).await?;
        let available = catalog
            .definitions()
            .iter()
            .map(|s| json!({"id":s.id(),"description":s.description()}))
            .collect::<Vec<_>>();
        let runtime = RuntimeFactory::for_management(provider, registry)?;
        let guide = catalog
            .definitions()
            .iter()
            .find(|skill| skill.id() == "management-guide")
            .and_then(|skill| skill.files().get("SKILL.md"))
            .cloned()
            .unwrap_or_default();
        let instructions = agent_runtime::prompts::PromptStore::instance().read("management")?;
        let prompt = format!(
            "{instructions}\nCurrent project: {project}. Available management skills: {available:?}. Loaded management guide:\n{guide}\nPrevious records (may contain interrupted actions; inspect state before retry):\n{records}\nLatest human request:\n{content}"
        );
        let user =
            session::append(&self.core, id, json!({"type":"user","content":content})).await?;
        self.set_status(id, "running").await?;
        history["events"]
            .as_array_mut()
            .ok_or("invalid session")?
            .push(user);
        history["status"] = json!("running");
        self.journal.begin(id, history);
        let (cancel, cancelled) = watch::channel(false);
        active.insert(id, cancel);
        tokio::spawn(super::execution::run(
            self.clone(),
            self.journal.clone(),
            id,
            runtime,
            project,
            source,
            prompt,
            cancelled,
            crate::core::steering::Mailbox::new(project, "admin".into()),
        ));
        Ok(json!({"session_id":id,"status":"accepted"}))
    }
    pub(crate) fn subscribe(&self) -> tokio::sync::broadcast::Receiver<super::LiveEvent> {
        self.journal.subscribe()
    }
    pub(super) async fn finish_run(&self, id: Uuid) {
        self.active.lock().await.remove(&id);
    }
    async fn set_status(&self, id: Uuid, status: &str) -> Result<(), String> {
        self.core
            .state()
            .store
            .transaction(|d| {
                let mut row = d
                    .get("management_sessions", &id.to_string())
                    .cloned()
                    .ok_or("unknown session")?;
                row["status"] = json!(status);
                d.set("management_sessions", &id.to_string(), row);
                Ok(())
            })
            .await
    }
}

#[cfg(test)]
mod locking_tests {
    use super::*;
    #[tokio::test]
    async fn status_releases_provider_before_waiting_for_active_sessions() {
        let manager = Manager::new(Core::new(crate::crabot_tests::state("status-locks").await));
        let _active = manager.active.lock().await;
        tokio::select! {
            biased;
            _ = manager.status() => panic!("active lock should be pending"),
            _ = async { assert!(manager.provider.try_lock().is_ok(), "status holds provider while waiting for active"); } => {},
        }
    }
}
