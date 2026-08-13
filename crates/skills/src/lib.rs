use anyhow::{anyhow, Result};
use crabot_domain::CapabilityRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub required_capabilities: Vec<CapabilityRef>,
    pub planner_hint: PlannerHint,
    pub approval: SkillApprovalPolicy,
    pub input_schema: serde_json::Value,
    pub output_contract: String,
}

impl SkillManifest {
    pub fn command_names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlannerHint {
    ManualOnly,
    AutoCallable,
    PlannerEntrypoint,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SkillApprovalPolicy {
    Never,
    OnSideEffect,
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub kind: SlashCommandKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SlashCommandKind {
    Plan,
    Run,
    Tools,
    Skills,
    Status,
    Approve,
    Reject,
    Replay,
    Skill,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandInvocation {
    pub command: String,
    pub args: String,
    pub kind: SlashCommandKind,
}

#[derive(Debug, Clone, Default)]
pub struct SkillCatalog {
    skills: BTreeMap<String, SkillManifest>,
    commands: BTreeMap<String, SlashCommand>,
}

impl SkillCatalog {
    pub fn with_builtin_commands() -> Self {
        let mut catalog = Self::default();
        for command in builtin_commands() {
            catalog.register_command(command);
        }
        catalog
    }

    pub fn register_skill(&mut self, skill: SkillManifest) {
        for command_name in skill.command_names() {
            self.commands.insert(
                command_name.to_string(),
                SlashCommand {
                    name: command_name.to_string(),
                    description: skill.description.clone(),
                    kind: SlashCommandKind::Skill,
                },
            );
        }
        self.skills.insert(skill.name.clone(), skill);
    }

    pub fn register_command(&mut self, command: SlashCommand) {
        self.commands.insert(command.name.clone(), command);
    }

    pub fn skills(&self) -> impl Iterator<Item = &SkillManifest> {
        self.skills.values()
    }

    pub fn commands(&self) -> impl Iterator<Item = &SlashCommand> {
        self.commands.values()
    }

    pub fn parse(&self, input: &str) -> Result<CommandInvocation> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return Err(anyhow!("slash command must start with '/'"));
        }

        let body = trimmed.trim_start_matches('/');
        let mut parts = body.splitn(2, char::is_whitespace);
        let command = parts.next().unwrap_or_default();
        let args = parts.next().unwrap_or_default().trim().to_string();

        let registered = self
            .commands
            .get(command)
            .ok_or_else(|| anyhow!("unknown slash command: /{}", command))?;

        Ok(CommandInvocation {
            command: command.to_string(),
            args,
            kind: registered.kind,
        })
    }
}

pub fn builtin_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand {
            name: "plan".to_string(),
            description: "生成或刷新 Task DAG，但不执行".to_string(),
            kind: SlashCommandKind::Plan,
        },
        SlashCommand {
            name: "run".to_string(),
            description: "执行当前 plan 或一次性任务".to_string(),
            kind: SlashCommandKind::Run,
        },
        SlashCommand {
            name: "tools".to_string(),
            description: "列出当前 session 可用工具、MCP 与插件能力".to_string(),
            kind: SlashCommandKind::Tools,
        },
        SlashCommand {
            name: "skills".to_string(),
            description: "列出 skill catalog".to_string(),
            kind: SlashCommandKind::Skills,
        },
        SlashCommand {
            name: "status".to_string(),
            description: "查看 session、department、agent、task 状态".to_string(),
            kind: SlashCommandKind::Status,
        },
        SlashCommand {
            name: "approve".to_string(),
            description: "批准等待中的高风险任务或工具调用".to_string(),
            kind: SlashCommandKind::Approve,
        },
        SlashCommand {
            name: "reject".to_string(),
            description: "拒绝等待中的高风险任务或工具调用".to_string(),
            kind: SlashCommandKind::Reject,
        },
        SlashCommand {
            name: "replay".to_string(),
            description: "回放一次 session 事件流".to_string(),
            kind: SlashCommandKind::Replay,
        },
    ]
}

pub fn default_skill_catalog() -> SkillCatalog {
    let mut catalog = SkillCatalog::with_builtin_commands();
    catalog.register_skill(SkillManifest {
        name: "architect".to_string(),
        description: "将目标拆成可执行计划、风险点和验收标准".to_string(),
        aliases: vec!["design".to_string()],
        required_capabilities: Vec::new(),
        planner_hint: PlannerHint::PlannerEntrypoint,
        approval: SkillApprovalPolicy::Never,
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "objective": { "type": "string" } },
            "required": ["objective"]
        }),
        output_contract: "Task DAG + acceptance criteria".to_string(),
    });
    catalog.register_skill(SkillManifest {
        name: "toolsmith".to_string(),
        description: "发现、解释和组合可用工具/MCP/插件能力".to_string(),
        aliases: vec!["capabilities".to_string()],
        required_capabilities: Vec::new(),
        planner_hint: PlannerHint::AutoCallable,
        approval: SkillApprovalPolicy::OnSideEffect,
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "goal": { "type": "string" } },
            "required": ["goal"]
        }),
        output_contract: "Selected capabilities with approval requirements".to_string(),
    });
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_builtin_command() {
        let catalog = SkillCatalog::with_builtin_commands();
        let invocation = catalog.parse("/plan build a tool").unwrap();
        assert_eq!(invocation.command, "plan");
        assert_eq!(invocation.args, "build a tool");
        assert_eq!(invocation.kind, SlashCommandKind::Plan);
    }

    #[test]
    fn registers_skill_alias_as_command() {
        let catalog = default_skill_catalog();
        let invocation = catalog.parse("/design hello").unwrap();
        assert_eq!(invocation.kind, SlashCommandKind::Skill);
    }
}
