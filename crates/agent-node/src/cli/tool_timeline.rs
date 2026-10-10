use serde_json::Value;
#[derive(Default)]
pub(super) struct ToolTimeline {
    entries: Vec<Entry>,
    current_start: usize,
}
struct Entry {
    id: Option<String>,
    name: String,
    input: String,
    output: String,
    pending: bool,
}
fn shown(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        s.into()
    } else {
        serde_json::to_string_pretty(v).unwrap_or_default()
    }
}
impl ToolTimeline {
    pub fn begin_turn(&mut self) {
        self.current_start = self.entries.len();
    }
    pub fn status(&self, step: usize) -> Option<String> {
        let entries = &self.entries[self.current_start.min(self.entries.len())..];
        let pending = entries.iter().filter(|e| e.pending).collect::<Vec<_>>();
        let items = if pending.is_empty() {
            entries.iter().rev().take(3).collect::<Vec<_>>()
        } else {
            pending
        };
        if items.is_empty() {
            return None;
        }
        let e = items[step % items.len()];
        let input = serde_json::from_str::<Value>(&e.input).unwrap_or_default();
        let name = super::tool_summary::summary(&e.name, &input);
        Some(format!(
            "{} · {}",
            if e.pending { "执行中" } else { "已完成" },
            name
        ))
    }

    pub fn record(&mut self, row: &Value) -> Option<String> {
        let event = row.get("payload").unwrap_or(row);
        let kind = event["type"].as_str().unwrap_or("");
        if !matches!(
            kind,
            "tool_started" | "agent.tool.started" | "tool_finished" | "agent.tool.finished"
        ) {
            return None;
        }
        let parsed = event["content"]
            .as_str()
            .or_else(|| event["text"].as_str())
            .and_then(|s| serde_json::from_str::<Value>(s).ok());
        let data = parsed.as_ref().unwrap_or(event);
        let id = data["call_id"]
            .as_str()
            .or_else(|| data["id"].as_str())
            .map(str::to_owned);
        if kind.ends_with("started") {
            self.entries.push(Entry {
                id,
                name: data["name"].as_str().unwrap_or("tool").into(),
                input: shown(
                    data.get("input")
                        .or_else(|| data.get("arguments"))
                        .unwrap_or(&Value::Null),
                ),
                output: String::new(),
                pending: true,
            });
            Some(String::new())
        } else {
            if let Some(entry) = self
                .entries
                .iter_mut()
                .find(|e| e.pending && id.as_ref().is_none_or(|id| e.id.as_ref() == Some(id)))
            {
                entry.output = shown(
                    data.get("output")
                        .or_else(|| data.get("content"))
                        .unwrap_or(data),
                );
                if matches!(entry.input.as_str(), "null" | "{}" | "") {
                    if let Ok(result) = serde_json::from_str::<Value>(&entry.output) {
                        if let Some(input) = result.get("input") {
                            entry.input = shown(input);
                        }
                    }
                }
                if entry.output.len() > 65536 {
                    let mut at = 65536;
                    while !entry.output.is_char_boundary(at) {
                        at -= 1
                    }
                    entry.output.truncate(at);
                    entry.output.push_str("\n…输出已截断");
                }
                entry.pending = false;
                return Some(String::new());
            }
            Some(String::new())
        }
    }
    pub fn details(&self, index: Option<usize>) -> String {
        let Some((number, entry)) = index
            .and_then(|n| n.checked_sub(1))
            .or_else(|| self.entries.len().checked_sub(1))
            .and_then(|i| self.entries.get(i).map(|entry| (i + 1, entry)))
        else {
            return "暂无该工具调用记录。".into();
        };
        format!(
            "[工具 #{number}] {} · {}\n输入：\n{}\n输出：\n{}",
            entry.name,
            if entry.pending {
                "执行中"
            } else {
                "已完成"
            },
            entry.input,
            entry.output
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn one_row_by_default_and_details_are_expandable() {
        let mut tools = ToolTimeline::default();
        tools.record(&json!({"type":"tool_started","name":"long_tool","call_id":"x","input":{"secret":"detail"}}));
        let line = tools
            .record(&json!({"type":"tool_finished","call_id":"x","output":"many\nlines"}))
            .unwrap();
        assert!(line.is_empty());
        assert!(tools.status(0).unwrap().contains("已完成"));
        assert!(tools.details(None).contains("many\nlines"));
        tools.begin_turn();
        assert!(tools.status(0).is_none());
    }

    #[test]
    fn legacy_execute_result_restores_missing_code_for_the_status_line() {
        let mut tools = ToolTimeline::default();
        tools.record(&json!({"type":"tool_started","name":"execute","id":"call"}));
        tools.record(&json!({"type":"tool_finished","id":"call","output":json!({"name":"execute","input":{"code":"await tools.search({query: 'shell'})"},"output":"ok"}).to_string()}));
        assert_eq!(
            tools.status(0).as_deref(),
            Some("已完成 · execute · search · \"shell\"")
        );
    }

    #[test]
    fn live_and_completed_actions_keep_their_arguments_in_the_summary() {
        let mut tools = ToolTimeline::default();
        tools.record(&json!({"payload":{"type":"agent.tool.started","content":json!({"name":"find","call_id":"one","arguments":{"target":"history","query":"部署"}}).to_string()}}));
        assert_eq!(
            tools.status(0).as_deref(),
            Some("执行中 · find · history : \"部署\"")
        );
        tools.record(&json!({"type":"tool_finished","call_id":"one","output":"done"}));
        assert_eq!(
            tools.status(0).as_deref(),
            Some("已完成 · find · history : \"部署\"")
        );
        tools.record(&json!({"type":"tool_started","name":"shell","call_id":"two","input":{"command":"ls -la src"}}));
        assert_eq!(
            tools.status(0).as_deref(),
            Some("执行中 · shell · ls -la src")
        );
        assert!(tools.details(None).contains("ls -la src"));
    }
}
