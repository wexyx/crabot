use super::{SessionScope, process::Process};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};

pub struct ProcessSessions {
    processes: Mutex<HashMap<String, Arc<Process>>>,
}
impl ProcessSessions {
    pub fn global() -> &'static Self {
        static MANAGER: LazyLock<ProcessSessions> = LazyLock::new(ProcessSessions::new);
        &MANAGER
    }
    pub(crate) fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
        }
    }
    pub(crate) async fn start(
        &self,
        owner: SessionScope,
        root: PathBuf,
        command: String,
        interactive: bool,
        env: BTreeMap<String, String>,
        bridge: Option<crate::skills::SkillBridge>,
        wait_ms: u64,
    ) -> Result<Value, String> {
        let process = {
            let mut rows = self.processes.lock().unwrap();
            if rows.len() >= 64 {
                rows.retain(|_, p| p.running());
            }
            if rows.len() >= 64
                || rows.values().filter(|p| p.running()).count() >= 16
                || rows
                    .values()
                    .filter(|p| p.running() && p.owner() == &owner)
                    .count()
                    >= 4
            {
                return Err("too many process sessions; stop unused sessions first".into());
            }
            let p = Process::launch(owner, root, command, interactive, env, bridge)?;
            rows.insert(p.id().into(), p.clone());
            p
        };
        process.wait(wait_ms).await;
        Ok(process.view(0, false))
    }
    fn owned(&self, scope: &SessionScope, id: &str) -> Result<Arc<Process>, String> {
        self.processes
            .lock()
            .unwrap()
            .get(id)
            .filter(|p| p.owner() == scope)
            .cloned()
            .ok_or("unknown process session for this Agent and chat".into())
    }
    fn human(&self, project: &str, chat: &str, id: &str) -> Result<Arc<Process>, String> {
        self.processes
            .lock()
            .unwrap()
            .get(id)
            .filter(|p| p.owner().project() == project && p.owner().chat() == chat)
            .cloned()
            .ok_or("unknown process session for this chat".into())
    }
    pub(crate) async fn read(
        &self,
        scope: &SessionScope,
        id: &str,
        after: u64,
        wait: u64,
    ) -> Result<Value, String> {
        let p = self.owned(scope, id)?;
        p.wait(wait).await;
        Ok(p.view(after, false))
    }
    pub(crate) async fn write(
        &self,
        scope: &SessionScope,
        id: &str,
        data: &str,
    ) -> Result<Value, String> {
        let p = self.owned(scope, id)?;
        crate::workspace::confirm_process_input(p.root(), p.command(), data).await?;
        p.write(data, false).await?;
        Ok(json!({"session_id":id,"input_sent":true}))
    }
    pub(crate) async fn stop(&self, scope: &SessionScope, id: &str) -> Result<Value, String> {
        let p = self.owned(scope, id)?;
        p.stop();
        p.wait(1000).await;
        Ok(p.view(0, false))
    }
    pub fn list(&self, project: &str, chat: &str) -> Value {
        json!(
            self.processes
                .lock()
                .unwrap()
                .values()
                .filter(|p| p.owner().project() == project && p.owner().chat() == chat)
                .map(|p| {
                    let mut v = p.view(u64::MAX, true);
                    v["stdout"] = json!("");
                    v["stderr"] = json!("");
                    v
                })
                .collect::<Vec<_>>()
        )
    }
    pub fn human_read(
        &self,
        project: &str,
        chat: &str,
        id: &str,
        after: u64,
    ) -> Result<Value, String> {
        Ok(self.human(project, chat, id)?.view(after, true))
    }
    /// Only human adapters call this. Input is never echoed into conversation history.
    pub async fn human_write(
        &self,
        project: &str,
        chat: &str,
        id: &str,
        data: &str,
        private: bool,
    ) -> Result<Value, String> {
        self.human(project, chat, id)?.write(data, private).await?;
        Ok(json!({"session_id":id,"input_sent":true}))
    }
    pub async fn human_stop(&self, project: &str, chat: &str, id: &str) -> Result<Value, String> {
        let p = self.human(project, chat, id)?;
        p.stop();
        p.wait(1000).await;
        Ok(p.view(0, true))
    }
    pub(super) fn stop_owner(&self, owner: &SessionScope) {
        for p in self
            .processes
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.owner() == owner)
        {
            p.stop();
        }
    }
    pub fn stop_chat(&self, project: &str, chat: &str) {
        for p in self
            .processes
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.owner().project() == project && p.owner().chat() == chat)
        {
            p.stop();
        }
    }
    pub async fn shutdown(&self) {
        let rows = self
            .processes
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for p in &rows {
            p.stop();
        }
        for p in rows {
            p.wait(1000).await;
        }
    }
}
