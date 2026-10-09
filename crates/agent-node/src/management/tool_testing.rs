use super::Manager;
use agent_runtime::{
    skills::ExecutionPolicy,
    tools::{ToolContext, ToolFactory, ToolSession},
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;
impl Manager {
    pub(crate) async fn start_tool_test(
        self: &Arc<Self>,
        project: Uuid,
        scope: &str,
        agent: &str,
        input: Value,
    ) -> Result<Value, String> {
        // Resolve the same operator-owned target and registration policy as real execution.
        self.tool_settings(project, scope, agent).await?;
        if input.to_string().len() > 16384 {
            return Err("test arguments exceed 16 KiB".into());
        }
        let name = input["name"]
            .as_str()
            .ok_or("tool name required")?
            .to_owned();
        let args = input["arguments"].clone();
        if !args.is_object() {
            return Err("test arguments must be a JSON object".into());
        }
        let group = input["group"].as_str();
        self.capability_context(project, scope, agent, group)
            .await?;
        let workspace = self
            .core()
            .project_workspace(project, if scope == "business" { group } else { None })
            .await?;
        let registry = workspace
            .clone()
            .scope(async {
                let registry = if scope == "management" {
                    self.registry(project).await?
                } else {
                    let root = workspace
                        .workdir()
                        .map(std::path::Path::to_path_buf)
                        .or_else(|| std::env::var_os("AGENT_WORKDIR").map(std::path::PathBuf::from))
                        .unwrap_or_else(agent_runtime::paths::workdir);
                    ToolFactory::create(
                        ToolContext::new(
                            Some(root),
                            crate::core::skills::for_group(
                                self.core().state(),
                                project,
                                agent,
                                group,
                            )
                            .await?,
                            ExecutionPolicy::from_env()?,
                        )?
                        .with_tool_policy(
                            crate::core::tool_policy::for_group(
                                self.core().state(),
                                project,
                                "business",
                                agent,
                                group,
                            )
                            .await?,
                        ),
                    )?
                };
                Ok::<_, String>(registry)
            })
            .await?;
        if !registry.definitions().iter().any(|d| d.name() == name) {
            return Err("unknown or disabled tool; enable and save it before testing".into());
        }
        self.test_runs()
            .start(
                project,
                name.clone(),
                workspace.scope(async move {
                    let mut session = ToolSession::default();
                    registry.execute(&name, &args, &mut session).await
                }),
            )
            .await
    }
    pub(crate) async fn tool_test(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        self.core().project(project).await?;
        self.test_runs().read(project, id).await
    }
    pub(crate) async fn cancel_tool_test(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        self.core().project(project).await?;
        self.test_runs().cancel(project, id).await
    }
}
