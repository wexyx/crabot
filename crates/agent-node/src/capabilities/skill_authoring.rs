use super::Library;
use crate::storage::Store;
use agent_runtime::context::{HistoryFuture, SkillSource};
use serde_json::{Value, json};

/// The host chooses the scope. Models can author content, not grant permissions.
pub(crate) struct SkillAuthoring {
    library: Library,
    scope: &'static str,
}
impl SkillAuthoring {
    pub(crate) fn new(store: Store, scope: &'static str) -> Self {
        Self {
            library: Library::new(store),
            scope,
        }
    }
    async fn call(&self, input: Value) -> Result<Value, String> {
        let resources = self.library.resources(self.scope, "skill").await?;
        let existing = resources
            .iter()
            .find(|r| r.id == input["id"].as_str().unwrap_or_default());
        match input["action"].as_str() {
            Some("list") => Ok(json!(resources.iter().map(|r| json!({"id":r.id,"name":r.name(),"description":r.definition["description"],"enabled":r.definition["enabled"],"version":r.version,"readonly":r.readonly})).collect::<Vec<_>>())),
            Some("read") => {
                let row = existing.ok_or("unknown Skill library id")?;
                Ok(json!({"id":row.id,"definition":row.definition,"version":row.version,"readonly":row.readonly}))
            }
            Some("save") => {
                if input["deleted"] == true { return Err("Skill authoring cannot delete packages".into()); }
                let enabled = existing.is_some_and(|r| r.definition["enabled"] == true);
                if input["definition"]["enabled"].as_bool() != Some(enabled) {
                    return Err("New Skills must be disabled; preserve existing enabled state. Change enablement in Web.".into());
                }
                self.library.save(self.scope, "skill", input).await
            }
            _ => Err("Skill action must be list, read or save".into()),
        }
    }
}
impl SkillSource for SkillAuthoring {
    fn execute(&self, input: Value) -> HistoryFuture<'_> {
        Box::pin(self.call(input))
    }
}

#[cfg(test)]
#[path = "skill_authoring_tests.rs"]
mod tests;
