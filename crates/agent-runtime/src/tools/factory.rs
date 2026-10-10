use super::{Tool, ToolContext, ToolRegistry};
use std::sync::Arc;
pub struct ToolRegistration {
    pub scope: &'static str,
    pub create: fn(Arc<ToolContext>) -> Option<Arc<dyn Tool>>,
}
inventory::collect!(ToolRegistration);
pub struct ToolFactory;
impl ToolFactory {
    pub fn create(context: ToolContext) -> Result<ToolRegistry, String> {
        Self::create_scoped(context, "business")
    }
    pub fn create_scoped(context: ToolContext, scope: &str) -> Result<ToolRegistry, String> {
        if !matches!(scope, "business" | "management") {
            return Err("unknown tool scope".into());
        }
        let context = Arc::new(context);
        context.tool_policy.validate()?;
        let mut registry = ToolRegistry::new();
        for registration in inventory::iter::<ToolRegistration> {
            if registration.scope != scope && registration.scope != "shared" {
                continue;
            }
            if let Some(tool) = (registration.create)(context.clone()) {
                registry.register_builtin(tool)?;
            }
        }
        for definition in context.tool_policy.external() {
            if scope != "business" || context.workdir().is_none() {
                return Err("external commands require a business Agent workspace".into());
            }
            // Register even disabled entries first so they cannot shadow an internal tool.
            registry.register(Arc::new(super::external_command::CommandTool::new(
                definition.clone(),
                context.clone(),
            )))?;
            if !definition.enabled() {
                registry.unregister(definition.name());
            }
        }
        for name in context.tool_policy.disabled() {
            registry.unregister(name);
        }
        // Published last: `find_tools` must describe the registry the model will
        // actually be offered, not the pre-policy set.
        context.tool_index().publish(registry.definitions());
        registry.set_prompt(super::capability_prompt::CapabilityPrompt::new(context));
        Ok(registry)
    }
}
