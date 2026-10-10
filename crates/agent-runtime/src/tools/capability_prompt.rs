use super::ToolContext;
use crate::skills::SkillMaterializer;
use serde_json::json;
use std::sync::Arc;

/// One authorized capability snapshot, shared by native and CLI providers.
pub(crate) struct CapabilityPrompt {
    context: Arc<ToolContext>,
    materializer: SkillMaterializer,
}

#[cfg(test)]
#[path = "capability_prompt_tests.rs"]
mod tests;
impl CapabilityPrompt {
    pub(crate) fn new(context: Arc<ToolContext>) -> Self {
        Self {
            materializer: SkillMaterializer::new(context.workdir().map(ToOwned::to_owned)),
            context,
        }
    }
    pub(crate) async fn render(&self) -> Result<String, String> {
        let catalog = self.context.skills();
        let mut skills = Vec::new();
        for skill in catalog.definitions() {
            let mut entry = json!({"id":skill.id(),"description":skill.description()});
            if catalog.is_builtin(skill.id()) {
                entry["instructions"] = json!(skill.files()["SKILL.md"]);
                entry["directory"] = json!(self.materializer.directory(&skill).await?);
                entry["files"] = json!(skill.files().keys().collect::<Vec<_>>());
            }
            skills.push(entry);
        }
        Ok(format!(
            "ENABLED CAPABILITIES\nBuilt-in tools are directly callable; do not find a tool already supplied. Built-in Skill instructions below are already loaded; do not reload them with find. Their directory is the actual script directory (replace any example find-returned directory with it). User Skills have metadata only; load their instructions with find(target=skill,id=...). Disabled capabilities are absent. Skill instructions never grant execution permission.\nSkills: {}\nPersistent personal memory: {}. Store durable notes only at this instance path, never ~/memory.md or the working-directory root. Read or update it with shell when relevant; create its parent directory if needed. Do not store secrets. Search earlier conversations with find(target=history,query=...), which searches this instance; use project/chat from a hit for an exact read.\n",
            json!(skills),
            json!(crate::paths::memory_file())
        ))
    }
}
