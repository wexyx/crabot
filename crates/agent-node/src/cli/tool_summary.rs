use serde_json::Value;

const LIMIT: usize = 56;

pub(super) fn summary(name: &str, input: &Value) -> String {
    let parsed = input
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let input = parsed.as_ref().unwrap_or(input);
    let name = clean(name);
    let name = if name.is_empty() { "tool" } else { &name };
    let action = match name {
        "shell" => command_preview(input["command"].as_str().unwrap_or_default()),
        "find" => {
            let target = clean(input["target"].as_str().unwrap_or_default()).to_lowercase();
            let label = match target.as_str() {
                "tool" | "tools" => "工具",
                "skill" | "skills" => "Skill",
                "history" | "chat" => "历史",
                "doc" | "docs" => "文档",
                _ => "资源",
            };
            let subject = ["id", "name", "query"]
                .iter()
                .map(|key| clean(input[*key].as_str().unwrap_or_default()))
                .find(|s| !s.is_empty());
            format!(
                "{label} · {}",
                subject.as_deref().unwrap_or(if label == "历史" {
                    "最近记录"
                } else {
                    "列表"
                })
            )
        }
        _ => String::new(),
    };
    let text = if action.is_empty() {
        name.into()
    } else {
        format!("{name} · {action}")
    };
    let text = clean(&text);
    if text.chars().count() > LIMIT {
        format!("{}…", text.chars().take(LIMIT - 1).collect::<String>())
    } else {
        text
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sensitive(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    [
        "token",
        "password",
        "passwd",
        "secret",
        "api_key",
        "api-key",
        "apikey",
        "authorization",
    ]
    .iter()
    .any(|key| text.contains(key))
}

fn command_preview(command: &str) -> String {
    let command = clean(command);
    let mut result = Vec::new();
    let mut hide_next = false;
    for word in command.split_whitespace() {
        if hide_next {
            result.push("***".into());
            hide_next = false;
            continue;
        }
        if [
            "-H",
            "--header",
            "-d",
            "--data",
            "--data-raw",
            "-c",
            "--command",
            "-e",
            "--eval",
        ]
        .contains(&word)
        {
            result.push(word.into());
            result.push("…".into());
            break;
        }
        if let Some((key, _)) = word.split_once('=') {
            if !key.is_empty() && !word.contains("://") && sensitive(key) {
                result.push(format!("{key}=***"));
                continue;
            }
        }
        if (word.starts_with('-') && sensitive(word)) || matches!(word, "-u" | "--user") {
            result.push(word.into());
            hide_next = true;
            continue;
        }
        if let Some((scheme, address)) = word.split_once("://") {
            let address = address.split(['?', '#']).next().unwrap_or_default();
            let address = address
                .rsplit_once('@')
                .map(|(_, host)| format!("***@{host}"))
                .unwrap_or_else(|| address.into());
            result.push(format!("{scheme}://{address}"));
            continue;
        }
        let path = word.trim_matches(['\'', '"']);
        if path.starts_with('/') && path != "/" {
            let parts = path
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            result.push(if result.is_empty() {
                parts.last().unwrap_or(&path).to_string()
            } else if parts.len() > 2 {
                format!("…/{}", parts[parts.len() - 2..].join("/"))
            } else {
                path.into()
            });
        } else {
            result.push(word.into());
        }
    }
    result.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cli_and_web_share_concise_summary_examples() {
        let cases: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/tool-summaries.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(
                summary(case["name"].as_str().unwrap(), &case["input"]),
                case["expected"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn long_unicode_input_is_shortened_without_modifying_arguments() {
        let input = json!({"target":"history","query":"查🙂".repeat(100)});
        let before = input.clone();
        let text = summary("find", &input);
        assert_eq!(text.chars().count(), LIMIT);
        assert!(text.ends_with('…'));
        assert_eq!(input, before);
    }
}
