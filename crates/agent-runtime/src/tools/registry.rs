use super::{Tool, ToolDefinition, ToolSession};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
/// Clone creates a registration snapshot, not a shared mutable global registry.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, (ToolDefinition, Arc<dyn Tool>)>,
    builtins: BTreeSet<String>,
    prompt: Option<Arc<super::capability_prompt::CapabilityPrompt>>,
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
        self.builtins.remove(name);
        self.tools.remove(name).is_some()
    }
    pub(crate) fn register_builtin(&mut self, tool: Arc<dyn Tool>) -> Result<(), String> {
        let name = tool.definition().name().to_owned();
        self.register(tool)?;
        self.builtins.insert(name);
        Ok(())
    }
    pub(crate) fn set_prompt(&mut self, prompt: super::capability_prompt::CapabilityPrompt) {
        self.prompt = Some(Arc::new(prompt));
    }
    pub(crate) async fn instructions(&self) -> Result<String, String> {
        match &self.prompt {
            Some(prompt) => prompt.render().await,
            None => Ok(String::new()),
        }
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|(d, _)| d.clone()).collect()
    }
    /// Enabled built-ins are ready on the first turn. External tools are discoverable.
    pub fn advertised(&self, session: &ToolSession) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|(d, _)| {
                self.builtins.contains(d.name())
                    || super::exposure::is_discovery(d.name())
                    || session.is_unlocked(d.name())
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
                    self.builtins.contains(*name)
                        || super::exposure::is_discovery(name)
                        || session.is_unlocked(name)
                })
                .map(|(name, entry)| (name.clone(), entry.clone()))
                .collect(),
            builtins: self.builtins.clone(),
            prompt: self.prompt.clone(),
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
