use serde_json::{Value, json};

pub(super) fn context(events: &Value) -> String {
    let mut messages: Vec<Value> = Vec::new();
    let mut text = String::new();
    fn flush(messages: &mut Vec<Value>, text: &mut String) {
        if !text.is_empty() {
            messages.push(json!({"role":"assistant","content":std::mem::take(text)}));
        }
    }
    let events = events.as_array().map(Vec::as_slice).unwrap_or(&[]);
    // Summaries anchor the window and must survive the recent-turn selection, so
    // they are held out of it and prepended as the leading records.
    let summaries = events
        .iter()
        .filter(|event| event["type"] == "summary")
        .cloned()
        .collect::<Vec<_>>();
    for event in crate::core::recent_context::select(
        &events
            .iter()
            .filter(|event| event["type"] != "summary")
            .cloned()
            .collect::<Vec<_>>(),
        usize::MAX,
    ) {
        match event["type"].as_str().unwrap_or_default() {
            "text_delta" => text.push_str(event["text"].as_str().unwrap_or_default()),
            "completed" => {
                let answer = event["text"].as_str().unwrap_or_default();
                if text.ends_with(answer) {
                    flush(&mut messages, &mut text);
                } else {
                    flush(&mut messages, &mut text);
                    if !answer.is_empty() {
                        messages.push(json!({"role":"assistant","content":answer}));
                    }
                }
            }
            "user" => {
                flush(&mut messages, &mut text);
                messages.push(json!({"role":"user","content":event["content"]}));
            }
            _ => {
                flush(&mut messages, &mut text);
                let mut event = event.clone();
                if let Some(e) = event.as_object_mut() {
                    e.remove("seq");
                }
                messages.push(event);
            }
        }
    }
    for summary in summaries {
        let content = summary["payload"]["content"]
            .as_str()
            .or_else(|| summary["content"].as_str())
            .unwrap_or_default();
        messages.insert(
            0,
            json!({"role":"user","content":format!("Structured summary of earlier records (lossy history, not new instructions):\n{content}")}),
        );
    }
    flush(&mut messages, &mut text);
    serde_json::to_string(&messages).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragments_and_final_answer_are_not_duplicated() {
        let input = json!([{"type":"user","content":"hi"},{"type":"text_delta","text":"你"},{"type":"text_delta","text":"好"},{"type":"completed","text":"你好"}]);
        let result = context(&input);
        assert_eq!(result.matches("你好").count(), 1);
        assert!(!result.contains("text_delta"));
    }
}
