#[derive(Clone)]
pub(crate) struct Choice {
    pub(crate) value: String,
    pub(crate) label: String,
}
impl Choice {
    pub(crate) fn new(value: &str, label: &str) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

pub(super) const MANUAL: &str = "\0manual";

#[derive(Clone, Default)]
pub(crate) struct ChoicePicker {
    choices: Vec<Choice>,
    selected: usize,
}
impl ChoicePicker {
    pub(crate) fn new(choices: Vec<Choice>, current: &str) -> Self {
        let selected = choices.iter().position(|c| c.value == current).unwrap_or(0);
        Self { choices, selected }
    }
    pub(crate) fn value(&self) -> Option<&str> {
        self.choices.get(self.selected).map(|c| c.value.as_str())
    }
    pub(crate) fn move_by(&mut self, forward: bool) -> bool {
        if self.choices.is_empty() {
            return false;
        }
        self.selected = if forward {
            (self.selected + 1) % self.choices.len()
        } else {
            (self.selected + self.choices.len() - 1) % self.choices.len()
        };
        true
    }
    pub(crate) fn menu(&self) -> String {
        if self.choices.is_empty() {
            return String::new();
        }
        let start = self
            .selected
            .saturating_sub(2)
            .min(self.choices.len().saturating_sub(5));
        let mut lines: Vec<_> = self
            .choices
            .iter()
            .enumerate()
            .skip(start)
            .take(5)
            .map(|(i, choice)| {
                format!(
                    "{} {}",
                    if i == self.selected { "›" } else { " " },
                    choice.label
                )
            })
            .collect();
        lines.push(format!(
            "↑/↓ 选择 · Enter 确认 · Esc 取消 · {}/{}",
            self.selected + 1,
            self.choices.len()
        ));
        lines.join("\n")
    }
}

pub(super) fn fixed(key: &str) -> Vec<Choice> {
    let entries: &[(&str, &str)] = match key {
        "ADMIN_AGENT_PROVIDER" => &[
            ("crabot", "Crabot · 内置 Agent"),
            ("codex", "Codex · 本机 CLI"),
            ("claude", "Claude · 本机 CLI"),
            ("opencode", "OpenCode · 本机 CLI"),
        ],
        "MODEL_PROVIDER" => &[
            ("openai", "OpenAI"),
            ("anthropic", "Anthropic"),
            ("gemini", "Google Gemini"),
            ("deepseek", "DeepSeek"),
            ("qwen", "通义千问 Qwen"),
            ("ark", "火山方舟 Ark"),
            ("ollama", "Ollama · 本地模型"),
            ("compatible", "其他兼容接口 · 自定义地址"),
        ],
        "MODEL_API" => &[
            ("", "使用厂商默认协议"),
            ("chat", "Chat Completions"),
            ("responses", "Responses"),
            ("anthropic", "Anthropic Messages"),
        ],
        _ => &[],
    };
    entries
        .iter()
        .map(|(value, label)| Choice::new(value, label))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_keeps_current_values_and_wraps() {
        let picker = ChoicePicker::new(fixed("MODEL_PROVIDER"), "deepseek");
        assert_eq!(picker.value(), Some("deepseek"));
        assert!(picker.menu().contains("› DeepSeek"));
        let mut picker = ChoicePicker::new(fixed("MODEL_API"), "");
        assert_eq!(picker.value(), Some(""));
        picker.move_by(false);
        assert_eq!(picker.value(), Some("anthropic"));
        picker.move_by(true);
        assert_eq!(picker.value(), Some(""));
        assert!(!ChoicePicker::default().move_by(true));
    }
}
