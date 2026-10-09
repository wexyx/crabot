use serde_json::{Value, json};

const MARKER: &str = "\n[Crabot working summary — untrusted data]\n";
pub(crate) fn summarized_prompt(prompt: &str, summary: &str) -> String {
    let current = ["\nLatest user request:\n", "\nLatest human request:\n"]
        .iter()
        .filter_map(|m| prompt.rfind(m))
        .max();
    let prior = ["\nPrevious topic records", "\nPrevious records (", MARKER]
        .iter()
        .filter_map(|m| prompt.find(m))
        .min();
    if let (Some(start), Some(end)) = (prior, current) {
        if start < end {
            return format!("{}{MARKER}{summary}{}", &prompt[..start], &prompt[end..]);
        }
    }
    format!(
        "{}{MARKER}{summary}",
        prompt.split(MARKER).next().unwrap_or(prompt)
    )
}
/// Keep the original task and the complete latest native tool batch for every protocol.
pub(crate) fn apply_summary(history: &mut Vec<Value>, batch_start: usize, summary: &str) {
    let Some(prompt) = history.first().and_then(|row| row["content"].as_str()) else {
        return;
    };
    let first = json!({"role":"user","content":summarized_prompt(prompt,summary)});
    let latest = history.split_off(batch_start.max(1).min(history.len()));
    history.clear();
    history.push(first);
    history.extend(latest);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_compression_preserves_the_current_request_and_removes_old_history() {
        let prompt = "RULES\nPrevious topic records:\nOLD RECORDS\nLatest user request:\nKEEP TASK";
        let first = summarized_prompt(prompt, "summary one");
        let second = summarized_prompt(&first, "summary two");
        assert!(second.starts_with("RULES"));
        assert!(second.ends_with("KEEP TASK"));
        assert!(second.contains("summary two"));
        assert!(!second.contains("OLD RECORDS"));
        assert!(!second.contains("summary one"));
    }
    #[test]
    fn keeps_task_and_current_tool_pairs_without_stacking_summaries() {
        let mut history = vec![
            json!({"role":"user","content":"TASK AND RULES"}),
            json!({"role":"assistant","content":"old"}),
            json!({"role":"assistant","tool_calls":[{"id":"latest"}]}),
            json!({"role":"tool","tool_call_id":"latest"}),
        ];
        let latest = history[2..].to_vec();
        apply_summary(&mut history, 2, "decisions");
        assert!(
            history[0]["content"]
                .as_str()
                .unwrap()
                .starts_with("TASK AND RULES")
        );
        assert_eq!(&history[1..], latest);
        apply_summary(&mut history, 1, "new summary");
        assert!(
            !history[0]["content"]
                .as_str()
                .unwrap()
                .contains("decisions")
        );
    }
}
