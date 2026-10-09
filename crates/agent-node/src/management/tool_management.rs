use super::Manager;
use agent_runtime::{
    skills::ExecutionPolicy,
    tools::{ToolContext, ToolFactory, ToolPolicy},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
impl Manager {
    async fn tool_target(&self, p: Uuid, scope: &str, agent: &str) -> Result<(), String> {
        self.core().project(p).await?;
        match scope {
            "management" if agent == "admin" => Ok(()),
            "business"
                if self
                    .core()
                    .state()
                    .store
                    .get("local_agents", &format!("{p}:{agent}"))
                    .await
                    .is_some() =>
            {
                Ok(())
            }
            _ => Err("unknown local Agent or tool scope".into()),
        }
    }
    pub(crate) async fn tool_settings(
        self: &Arc<Self>,
        p: Uuid,
        scope: &str,
        agent: &str,
    ) -> Result<Value, String> {
        self.tool_target(p, scope, agent).await?;
        let row = self
            .core()
            .state()
            .store
            .get("tool_policies", &format!("{p}:{scope}:{agent}"))
            .await
            .unwrap_or(json!({"version":0,"policy":ToolPolicy::default()}));
        let catalog = self.tool_catalog(p).await?;
        Ok(
            json!({"version":row["version"],"policy":row["policy"],"builtins":catalog[scope],"note":"下一轮执行生效；内置工具不可删除。此策略只控制 Crabot 注册工具，不控制供应商 CLI 自带工具。外部命令固定内容、无参数插值，每次执行仍需人类确认。"}),
        )
    }
    pub(crate) async fn save_tool_settings(
        self: &Arc<Self>,
        p: Uuid,
        scope: &str,
        agent: &str,
        input: Value,
    ) -> Result<Value, String> {
        self.tool_target(p, scope, agent).await?;
        let policy: ToolPolicy =
            serde_json::from_value(input["policy"].clone()).map_err(|e| e.to_string())?;
        policy.validate()?;
        let catalog = self.tool_catalog(p).await?;
        let names = catalog[scope]
            .as_array()
            .ok_or("invalid scope")?
            .iter()
            .filter_map(|v| v["name"].as_str())
            .collect::<Vec<_>>();
        if policy
            .disabled()
            .iter()
            .any(|n| !names.contains(&n.as_str()))
        {
            return Err("disabled list may only contain registered built-in tools".into());
        }
        if scope == "management" && !policy.external().is_empty() {
            return Err("external commands belong to business Agents only".into());
        }
        if scope == "business" {
            let root = agent_runtime::paths::workdir();
            ToolFactory::create(
                ToolContext::new(
                    Some(root),
                    crate::core::skills::snapshot(self.core().state(), p).await?,
                    ExecutionPolicy::from_env()?,
                )?
                .with_tool_policy(policy.clone()),
            )?;
        }
        let version = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let key = format!("{p}:{scope}:{agent}");
        self.core().state().store.transaction(|data|{
            let previous=data.get("tool_policies",&key).and_then(|r|r["version"].as_u64()).unwrap_or(0);
            if previous!=version{return Err("version_conflict: reload tool settings before saving".into())}
            let row=json!({"project_id":p,"scope":scope,"agent":agent,"version":version.checked_add(1).ok_or("version overflow")?,"policy":policy});
            data.set("tool_policies",&key,row.clone());Ok(row)
        }).await
    }
}
