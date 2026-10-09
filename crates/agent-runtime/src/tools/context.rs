use crate::skills::{ExecutionPolicy, SkillCatalog};
use std::path::PathBuf;
pub struct ToolContext {
    pub(crate) tool_policy: super::ToolPolicy,
    pub(crate) execution: ExecutionPolicy,
    extension: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    pub(crate) root: Option<PathBuf>,
    pub(crate) catalog: SkillCatalog,
    index: super::ToolIndex,
}
impl ToolContext {
    pub fn with_tool_policy(mut self, policy: super::ToolPolicy) -> Self {
        self.tool_policy = policy;
        self
    }
    /// Host services injected by the composition root, never by model arguments.
    pub fn with_extension<T: Send + Sync + 'static>(mut self, value: std::sync::Arc<T>) -> Self {
        self.extension = Some(value);
        self
    }
    pub fn extension<T: Send + Sync + 'static>(&self) -> Option<std::sync::Arc<T>> {
        self.extension.clone()?.downcast().ok()
    }
    pub fn workdir(&self) -> Option<&std::path::Path> {
        self.root.as_deref()
    }
    pub fn skills(&self) -> &SkillCatalog {
        &self.catalog
    }
    /// The mirror `find_tools` searches. Filled in by the factory once the registry
    /// is complete, so it is empty for a context that never became one.
    pub fn tool_index(&self) -> &super::ToolIndex {
        &self.index
    }
    pub fn new(
        root: Option<PathBuf>,
        catalog: SkillCatalog,
        policy: ExecutionPolicy,
    ) -> Result<Self, String> {
        let root = root
            .map(|p| p.canonicalize().map_err(|e| e.to_string()))
            .transpose()?;
        Ok(Self {
            tool_policy: Default::default(),
            execution: policy.clone(),
            extension: None,
            root: root.clone(),
            catalog,
            index: super::ToolIndex::new(),
        })
    }
}
