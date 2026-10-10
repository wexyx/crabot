use super::{ToolContext, ToolSession};
use serde_json::{Value, json};
use std::sync::Arc;

struct Skill;
#[crate::tools::tool(scope="shared", name="skill", description="Create or edit user Skill packages in this instance's shared library. action=list returns IDs/versions including disabled skills; read loads a definition by library id; save creates (expected_version=0) or updates (id and expected_version from read). definition contains id, description, enabled, allow_python=false, files (relative UTF-8 paths including SKILL.md). New skills must be disabled; enabling and scope changes remain human-managed in Web. Existing enabled state must be preserved. Built-in skills are read-only. This tool writes registered packages to the instance skills/user directory; writing arbitrary folders alone does not register skills.", parameters=json!({"type":"object","properties":{"action":{"type":"string","enum":["list","read","save"]},"id":{"type":"string"},"expected_version":{"type":"integer","minimum":0},"definition":{"type":"object","properties":{"id":{"type":"string"},"description":{"type":"string"},"enabled":{"type":"boolean"},"allow_python":{"type":"boolean"},"files":{"type":"object","additionalProperties":{"type":"string"}}},"required":["id","description","files","enabled"],"additionalProperties":false}},"required":["action"],"additionalProperties":false}), runtime=crate)]
impl Skill {
    fn new(_: Arc<ToolContext>) -> Option<Self> {
        Some(Self)
    }
    async fn execute(&self, input: &Value, _: &mut ToolSession) -> Result<Value, String> {
        crate::context::SkillAccess::execute(input.clone()).await
    }
}
