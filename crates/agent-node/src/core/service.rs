use crate::{AppState, control, local, policies, storage};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Application capabilities shared by terminal, management tools and HTTP adapters.
#[derive(Clone)]
pub(crate) struct Core {
    state: AppState,
    lifecycle: Arc<Mutex<()>>,
    remote_directory: Arc<super::remote_directory::RemoteDirectory>,
}
impl Core {
    pub(crate) fn new(state: AppState) -> Self {
        Self {
            state,
            lifecycle: Arc::new(Mutex::new(())),
            remote_directory: super::remote_directory::RemoteDirectory::new(),
        }
    }
    pub(super) fn remote_directory(&self) -> &Arc<super::remote_directory::RemoteDirectory> {
        &self.remote_directory
    }
    pub(super) fn lifecycle_lock(&self) -> &Mutex<()> {
        &self.lifecycle
    }
    pub(crate) fn state(&self) -> &AppState {
        &self.state
    }
    pub(crate) async fn project(&self, project: Uuid) -> Result<(), String> {
        self.state
            .store
            .get("projects", &project.to_string())
            .await
            .ok_or("unknown project")?;
        Ok(())
    }
    pub(crate) async fn bootstrap(&self) -> Result<Uuid, String> {
        let _guard = self.lifecycle.lock().await;
        if let Some(row) = self.state.store.get("settings", "cli_project").await {
            let id =
                serde_json::from_value(row["project_id"].clone()).map_err(|e| format!("{e}"))?;
            self.project(id).await?;
            return Ok(id);
        }
        let space = Uuid::new_v4();
        let project = Uuid::new_v4();
        self.state
            .store
            .transaction(|data| {
                data.insert(
                    "spaces",
                    &space.to_string(),
                    json!({"id":space,"name":"Local","created_at":storage::now()}),
                )?;
                data.insert(
                    "projects",
                    &project.to_string(),
                    json!({"id":project,"space_id":space,"name":"CLI","created_at":storage::now()}),
                )?;
                data.insert("settings", "cli_project", json!({"project_id":project}))
            })
            .await?;
        Ok(project)
    }
    #[cfg(test)]
    pub(crate) async fn agents(&self, project: Uuid) -> Result<Value, String> {
        self.project(project).await?;
        Ok(json!(self.state.clients.lock().await.iter().filter(|((p,_),_)|*p==project)
            .map(|((_,id), c)|json!({"id":id,"role":c.role,"online":!c.sender.is_closed(),"node_id":c.node_id}))
            .collect::<Vec<_>>()))
    }
    pub(crate) async fn agent_start(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        let _guard = self.lifecycle.lock().await;
        let child = serde_json::from_value(input.clone()).map_err(|e| format!("{e}"))?;
        let id = input["client_id"].as_str().ok_or("client_id required")?;
        if id == "default" {
            return Err("默认 Agent 由启动配置维护，请在默认 Agent 详情修改".into());
        }
        let key = format!("{project}:{id}");
        if let Some(mut row) = self.state.store.get("local_agents", &key).await {
            if self
                .state
                .clients
                .lock()
                .await
                .contains_key(&(project, id.into()))
            {
                return Err("agent already started".into());
            }
            if row["provider"] != input["provider"] || row["role"] != input["role"] {
                return Err("existing agent configuration differs".into());
            }
            row["enabled"] = json!(true);
            self.state
                .store
                .transaction(|d| {
                    d.set("local_agents", &key, row);
                    Ok(())
                })
                .await?;
            local::start(self.state.clone(), project, child).await;
            return Ok(json!({"client_id":id,"status":"started"}));
        }
        local::register(&self.state, project, child).await
    }
    pub(crate) async fn configure_stopped_agent(
        &self,
        project: Uuid,
        id: &str,
        provider: &str,
    ) -> Result<Value, String> {
        self.project(project).await?;
        if id == "default" {
            return Err("默认 Agent 请通过模型配置修改".into());
        }
        provider.parse::<agent_runtime::RuntimeKind>()?;
        let _guard = self.lifecycle.lock().await;
        if self
            .state
            .clients
            .lock()
            .await
            .contains_key(&(project, id.into()))
        {
            return Err("Agent 正在运行。先向 默认 Agent 请求停止并完成人工确认，再执行配置命令；不会自动中断任务。".into());
        }
        let key = format!("{project}:{id}");
        self.state
            .store
            .transaction(|data| {
                let mut row = data
                    .get("local_agents", &key)
                    .cloned()
                    .ok_or("not a local agent")?;
                row["provider"] = json!(provider);
                row["version"] = json!(
                    row["version"]
                        .as_u64()
                        .unwrap_or(0)
                        .checked_add(1)
                        .ok_or("version overflow")?
                );
                row["enabled"] = json!(false);
                data.set("local_agents", &key, row);
                Ok(())
            })
            .await?;
        Ok(
            json!({"message":format!("已配置 {id} 的 Provider 为 {provider}；使用 /agent {id} start 启动。")}),
        )
    }
    pub(crate) async fn agent_stop(&self, project: Uuid, id: &str) -> Result<Value, String> {
        if id == "default" {
            return Err("默认 Agent 随 Crabot 生命周期运行；要取消当前任务请中断会话".into());
        }
        let _guard = self.lifecycle.lock().await;
        self.project(project).await?;
        let key = format!("{project}:{id}");
        self.state
            .store
            .transaction(|d| {
                let mut row = d
                    .get("local_agents", &key)
                    .cloned()
                    .ok_or("not a local agent")?;
                row["enabled"] = json!(false);
                d.set("local_agents", &key, row);
                Ok(())
            })
            .await?;
        self.state
            .clients
            .lock()
            .await
            .remove(&(project, id.into()));
        Ok(json!({"client_id":id,"status":"stopped","in_flight_tasks":"continue"}))
    }
    pub(crate) async fn control(
        &self,
        project: Uuid,
        op: &str,
        input: Value,
    ) -> Result<Value, String> {
        self.project(project).await?;
        control::call(&self.state, project, vec![], op.into(), input, vec![]).await
    }
    pub(crate) async fn group_chat(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        let mut input = input;
        let group_id = input["group_id"]
            .as_str()
            .ok_or("group_id required")?
            .to_owned();
        let content = input["content"]
            .as_str()
            .ok_or("content required")?
            .to_owned();
        if content.trim().is_empty() || content.len() > 192 * 1024 {
            return Err("content required, maximum 192 KiB".into());
        }
        // `@name` is resolved against this group's own roster before the run starts, so
        // the request reaches the Agent it names instead of the whole group. The title
        // keeps the original text: an `@name` there still describes the message.
        let policy: policies::Policy = serde_json::from_value(
            self.state()
                .policy_store
                .get(project, "group", &group_id)
                .await?
                .body["policy"]
                .clone(),
        )
        .map_err(|e| e.to_string())?;
        let found = super::mentions::find(&content, &policy.members);
        if !found.is_empty() {
            // The log keeps what the human wrote, `@name` included, because that is
            // the transcript; only the text the addressed Agents read drops the address.
            let stripped = super::mentions::strip(&content, &found);
            if stripped.is_empty() {
                return Err("mention needs a request; 例如：@alice 请检查这个错误".into());
            }
            input["prompt"] = json!(stripped);
            input["mentions"] = json!(found.iter().map(|m| &m.path).collect::<Vec<_>>());
        }
        super::project_titles::from_message(&self.state, project, &group_id, &content).await?;
        input["project_id"] = json!(project);
        let _guard = self.lifecycle_lock().lock().await;
        policies::begin_group(
            &self.state,
            serde_json::from_value(input).map_err(|e| e.to_string())?,
        )
        .await
    }
    pub(crate) async fn history(&self, project: Uuid, session: Uuid) -> Result<Value, String> {
        self.project(project).await?;
        let run = self.state.store.get("runs", &session.to_string()).await;
        let chat = run
            .as_ref()
            .and_then(|r| r["group_id"].as_str())
            .map(|g| format!("group:{g}"))
            .unwrap_or_else(|| format!("session:{session}"));
        // Read from the knowledge index for sequential history (display view).
        let history =
            crate::core::indexed_history::IndexedHistory::new(project, chat.clone(), None);
        let (rows, _) = history.rows(None, 0, u64::MAX, 10000).await?;
        let rows = if run.as_ref().is_some_and(|r| r["group_id"].is_string()) {
            rows.into_iter()
                .filter(|r| {
                    r["payload"]["message_id"] == json!(session)
                        || r["payload"]["session_id"] == json!(session)
                })
                .collect::<Vec<_>>()
        } else {
            rows
        };
        Ok(json!(rows))
    }
    pub(crate) async fn interrupt(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        self.project(project).await?;
        if !self
            .state
            .sessions
            .lock()
            .await
            .get(&id)
            .is_some_and(|s| s.project_id == project)
        {
            return Err("unknown session in selected project".into());
        }
        crate::conversation::interrupt_session(&self.state, id).await
    }
}
