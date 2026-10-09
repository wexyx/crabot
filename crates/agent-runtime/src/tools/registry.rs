use super::{Tool, ToolDefinition, ToolSession};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
/// Clone creates a registration snapshot, not a shared mutable global registry.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, (ToolDefinition, Arc<dyn Tool>)>,
}
impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), String> {
        let definition = tool.definition();
        let name = definition.name();
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(
                "tool names must use ASCII letters, numbers or underscores, at most 64 characters"
                    .into(),
            );
        }
        if self.tools.contains_key(name) {
            return Err(format!("duplicate tool: {name}"));
        }
        if definition.parameters()["type"] != "object" {
            return Err("tool parameters must be an object schema".into());
        }
        self.tools.insert(name.into(), (definition, tool));
        Ok(())
    }
    pub fn unregister(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|(d, _)| d.clone()).collect()
    }
    /// The definitions a model may see: the discovery entry points plus whatever it
    /// has revealed through `find_tools` during this run.
    pub fn advertised(&self, session: &ToolSession) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|(d, _)| {
                super::exposure::is_discovery(d.name()) || session.is_unlocked(d.name())
            })
            .map(|(d, _)| d.clone())
            .collect()
    }
    /// A registry restricted to what the model may see.
    ///
    /// A provider protocol takes a `&ToolRegistry` and reads `definitions()`, so
    /// handing it a projection keeps every protocol unchanged while hiding the rest
    /// of the toolset. Execution still runs against the full registry: discovery
    /// decides what is *shown*, and the policy layer already removed what is not
    /// allowed.
    pub fn exposed(&self, session: &ToolSession) -> Self {
        Self {
            tools: self
                .tools
                .iter()
                .filter(|(name, _)| {
                    super::exposure::is_discovery(name) || session.is_unlocked(name)
                })
                .map(|(name, entry)| (name.clone(), entry.clone()))
                .collect(),
        }
    }
    pub async fn execute(
        &self,
        name: &str,
        args: &Value,
        session: &mut ToolSession,
    ) -> Result<Value, String> {
        if !args.is_object() {
            return Err("tool arguments must be an object".into());
        }
        self.tools
            .get(name)
            .ok_or_else(|| format!("unknown or disabled tool: {name}"))?
            .1
            .execute(args, session)
            .await
    }
}
