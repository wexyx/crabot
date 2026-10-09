pub struct PromptDefinition {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub default: &'static str,
}

pub fn definitions() -> &'static [PromptDefinition] {
    &[
        PromptDefinition {
            id: "agent",
            title: "Crabot 基础指令",
            description: "内置 Harness 的系统提示词；显式 MODEL_SYSTEM_PROMPT 仍优先。",
            default: include_str!("../../../../conf/agent.md"),
        },
        PromptDefinition {
            id: "response",
            title: "默认回复要求",
            description: "各运行器共享；Agent 自定义回复要求优先。",
            default: include_str!("../../../../conf/response.md"),
        },
        PromptDefinition {
            id: "management",
            title: "管理对话",
            description: "管理会话的职责、工具和行为要求。",
            default: include_str!("../../../../conf/management.md"),
        },
        PromptDefinition {
            id: "discussion",
            title: "讨论模式",
            description: "职责、共识和协作规则；控制信号格式由程序追加。",
            default: include_str!("../../../../conf/discussion.md"),
        },
        PromptDefinition {
            id: "participation",
            title: "职责判断",
            description: "普通成员判断是否应参与本轮。",
            default: include_str!("../../../../conf/participation.md"),
        },
        PromptDefinition {
            id: "addressed",
            title: "明确指派",
            description: "用户点名后的回应要求；修改不会取消程序的禁止让出校验。",
            default: include_str!("../../../../conf/addressed.md"),
        },
        PromptDefinition {
            id: "participation-recheck",
            title: "让出复核",
            description: "申请让出后的复核要求；最多复核一次。",
            default: include_str!("../../../../conf/participation-recheck.md"),
        },
        PromptDefinition {
            id: "leader",
            title: "Leader 分工",
            description: "负责人分派任务的原则；JSON 协议由程序追加。",
            default: include_str!("../../../../conf/leader.md"),
        },
        PromptDefinition {
            id: "relay",
            title: "接力协商",
            description: "成员协商优先级的原则；评分协议由程序追加。",
            default: include_str!("../../../../conf/relay.md"),
        },
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_prompt_templates_are_english() {
        for definition in super::definitions() {
            assert!(
                definition.default.is_ascii(),
                "{} contains non-English template text",
                definition.id
            );
        }
    }
}
