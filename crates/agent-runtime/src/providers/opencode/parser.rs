//! Vendor event names stay at this boundary, never in AgentRuntime or gateway code.
//!
//! `opencode run --format json` flattens its event bus: the top-level `type` is the part
//! kind in snake_case (`text`, `tool_use`, `step_start`, `step_finish`) while the nested
//! `part.type` is kebab-case (`text`, `tool`, `step-start`, `step-finish`). Only the
//! top-level field is a stable discriminator.
//!
//! There is no delta field and no terminal event: a `text` part is republished with its
//! growing `text` while it streams, and the run ends when the process exits. Publishing
//! must therefore diff against what was already emitted for that part id, or the answer
//! accumulates the same prefix over and over.
use crate::{
    EventSink, RuntimeEvent,
    errors::{is_token_insufficient, token_limit_error},
};
use serde_json::Value;
use std::collections::HashMap;

fn failure(value: &Value) -> String {
    let error = &value["error"];
    let message = error["message"]
        .as_str()
        .or_else(|| error["data"]["message"].as_str())
        .or_else(|| error["name"].as_str())
        .or_else(|| value["message"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    // An exhausted output budget is a token limit, so relay may fail over. An exhausted
    // account balance is not: it must surface as itself instead of silently rotating.
    if error["name"] == "MessageOutputLengthError" || is_token_insufficient(&message) {
        token_limit_error(&message)
    } else {
        message
    }
}

#[derive(Default)]
pub struct EventParser {
    pub answer: String,
    pub error: Option<String>,
    /// Text already emitted per part id, so a republished part only adds its suffix.
    emitted: HashMap<String, usize>,
    /// Tool calls already announced, and already reported settled, keyed by call id.
    announced: std::collections::HashSet<String>,
    settled: std::collections::HashSet<String>,
    finished: bool,
}
impl EventParser {
    /// Feed one raw stdout line. Non-JSON is log output, not a failure.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn consume_line(&mut self, line: &str, events: &mut EventSink<'_>) {
        if let Ok(value) = serde_json::from_str::<Value>(line.trim()) {
            self.consume(&value, events);
        }
    }

    pub fn consume(&mut self, value: &Value, events: &mut EventSink<'_>) {
        if self.finished {
            return;
        }
        match value["type"].as_str().unwrap_or_default() {
            "error" => {
                self.error = Some(failure(value));
                self.finished = true;
            }
            "text" => self.consume_text(&value["part"], events),
            "reasoning" => {
                if let Some(text) = value["part"]["text"].as_str() {
                    self.append(
                        value["part"]["id"].as_str().unwrap_or_default(),
                        text,
                        events,
                        true,
                    );
                }
            }
            "tool_use" => self.consume_tool(&value["part"], events),
            // step_start / step_finish are progress, and a step that stops because the
            // output budget ran out must still surface as a token failure.
            "step_finish" => {
                if value["part"]["reason"] == "length" {
                    self.error = Some(token_limit_error(
                        "OpenCode stopped at the model's output token limit",
                    ));
                }
            }
            _ => (),
        }
    }
    fn consume_text(&mut self, part: &Value, events: &mut EventSink<'_>) {
        // A text part is republished with its growing content, so only the unstreamed
        // suffix is ever new. Republishing verbatim contributes nothing and needs no
        // special case: the suffix is empty.
        if let Some(text) = part["text"].as_str() {
            self.append(part["id"].as_str().unwrap_or_default(), text, events, false);
        }
    }
    /// Returns true when anything new was emitted for this part.
    fn append(
        &mut self,
        id: &str,
        text: &str,
        events: &mut EventSink<'_>,
        reasoning: bool,
    ) -> bool {
        let sent = self.emitted.get(id).copied().unwrap_or(0);
        // Deliberation is reported for folding and never becomes part of the answer.
        let suffix = if sent <= text.len() {
            &text[sent..]
        } else {
            text
        };
        if suffix.is_empty() {
            return false;
        }
        self.emitted.insert(id.into(), text.len());
        if !reasoning {
            self.answer.push_str(suffix);
        }
        push(events, suffix.to_owned(), reasoning);
        true
    }
    fn consume_tool(&mut self, part: &Value, events: &mut EventSink<'_>) {
        // Tool parts carry `partID`, not `id`; fall back to the call id.
        let id = part["id"]
            .as_str()
            .or_else(|| part["partID"].as_str())
            .or_else(|| part["callID"].as_str())
            .unwrap_or_default()
            .to_owned();
        if id.is_empty() {
            return;
        }
        let name = part["tool"].as_str().unwrap_or_default().to_owned();
        let status = part["state"]["status"].as_str().unwrap_or_default();
        // A call is often first seen already settled, so announcement and completion are
        // tracked separately: parts are republished and neither may fire twice.
        // `insert` is true only when the id was new, so each half fires exactly once.
        // A pending announcement may precede its arguments. Wait for actual input
        // or a running/settled part so the single start event carries usable detail.
        let ready = status != "pending"
            || part["state"]["input"]
                .as_object()
                .is_some_and(|input| !input.is_empty());
        if ready && self.announced.insert(id.clone()) {
            events(RuntimeEvent::ToolStarted {
                arguments: part["state"]["input"].clone(),
                id: id.clone(),
                name,
            });
        }
        if matches!(status, "completed" | "error") && self.settled.insert(id.clone()) {
            let mut output = serde_json::json!({
                "id": id,
                "name": part["tool"],
                "input": part["state"]["input"],
                "status": status,
            });
            output["output"] = part["state"]["output"].clone();
            events(RuntimeEvent::ToolFinished {
                id,
                output: serde_json::to_string(&output).unwrap_or_default(),
            });
        }
    }
}
fn push(events: &mut EventSink<'_>, text: String, reasoning: bool) {
    // Deliberation is reported on its own channel and must never join the answer.
    if reasoning {
        events(RuntimeEvent::ReasoningDelta { text });
    } else {
        events(RuntimeEvent::TextDelta { text });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Verbatim stdout of `opencode run --format json -m opencode/space-bunny-free --auto`
    /// reading notes.txt. Field names and value shapes are the real contract; only the
    /// timestamps and ids were shortened.
    const CAPTURED_TOOL_RUN: &str = r#"
{"type":"step_start","timestamp":1790748602614,"sessionID":"ses_x","part":{"id":"prt_a","sessionID":"ses_x","messageID":"msg_1","type":"step-start"}}
{"type":"text","timestamp":1790748602978,"sessionID":"ses_x","part":{"id":"prt_b","sessionID":"ses_x","messageID":"msg_1","type":"text","text":"I'll read the file first.","time":{"start":1790748602618,"end":1790748602978}}}
{"type":"tool_use","timestamp":1790748602989,"sessionID":"ses_x","part":{"partID":"prt_c","sessionID":"ses_x","messageID":"msg_1","type":"tool","id":"call_function_1","tool":"read","state":{"status":"completed","input":{"path":"notes.txt"},"output":"1: alpha\n2: beta\n3: gamma","title":"read","metadata":{"metadata":{"truncated":false}}},"time":{"start":1790748602969,"end":1790748602989}}}
{"type":"step_finish","timestamp":1790748602991,"sessionID":"ses_x","part":{"id":"prt_d","sessionID":"ses_x","messageID":"msg_1","type":"step-finish","reason":"tool-calls","cost":0,"tokens":{"input":3422,"output":69,"reasoning":0,"cache":{"read":7585,"write":0}}}}
{"type":"step_start","timestamp":1790748604330,"sessionID":"ses_x","part":{"id":"prt_e","sessionID":"ses_x","messageID":"msg_2","type":"step-start"}}
{"type":"text","timestamp":1790748605907,"sessionID":"ses_x","part":{"id":"prt_f_text-0","sessionID":"ses_x","messageID":"msg_2","type":"text","text":"**Line count: 3**","time":{"start":1790748603007,"end":1790748605907}}}
"#;

    fn replay(raw: &str) -> (String, Option<String>, Vec<RuntimeEvent>) {
        let mut parser = EventParser::default();
        let mut events = Vec::new();
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            // The runtime feeds raw stdout, so the parser must own log-line tolerance.
            parser.consume_line(line, &mut |e| events.push(e));
        }
        (parser.answer.clone(), parser.error.clone(), events)
    }

    #[test]
    fn a_captured_run_reproduces_its_answer_and_its_single_tool_call() {
        let (answer, error, events) = replay(CAPTURED_TOOL_RUN);
        assert_eq!(answer, "I'll read the file first.**Line count: 3**");
        assert!(error.is_none(), "{error:?}");
        let started = events
            .iter()
            .filter(|e| matches!(e, RuntimeEvent::ToolStarted { .. }))
            .count();
        let settled = events
            .iter()
            .filter(|e| matches!(e, RuntimeEvent::ToolFinished { .. }))
            .count();
        assert_eq!((started, settled), (1, 1), "{events:?}");
        let tool = events
            .iter()
            .find_map(|e| match e {
                RuntimeEvent::ToolStarted { name, .. } => Some(name.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(tool, "read");
    }

    #[test]
    fn a_captured_run_tolerates_log_lines_between_events() {
        let (answer, error, _) = replay(&format!(
            "INFO service started\n{CAPTURED_TOOL_RUN}WARN slow shutdown"
        ));
        assert_eq!(answer, "I'll read the file first.**Line count: 3**");
        assert!(error.is_none());
    }

    fn run(values: &[Value]) -> (String, Option<String>, Vec<RuntimeEvent>) {
        let mut parser = EventParser::default();
        let mut events = Vec::new();
        for value in values {
            parser.consume(value, &mut |e| events.push(e));
        }
        (parser.answer.clone(), parser.error.clone(), events)
    }

    #[test]
    fn a_settled_tool_observed_only_once_reports_both_halves() {
        let (answer, error, events) = run(&[
            json!({"type":"tool_use","part":{"partID":"prt_c","type":"tool","id":"call_1","tool":"read","state":{"status":"completed","input":{"path":"a"},"output":"alpha"}}}),
            // The same part republished must not announce or complete a second time.
            json!({"type":"tool_use","part":{"partID":"prt_c","type":"tool","id":"call_1","tool":"read","state":{"status":"completed","input":{"path":"a"},"output":"alpha"}}}),
            json!({"type":"text","part":{"id":"prt_d","type":"text","text":"done"}}),
        ]);
        assert_eq!(answer, "done");
        assert!(error.is_none());
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, RuntimeEvent::ToolStarted { .. }))
                .count(),
            1,
            "{events:?}"
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, RuntimeEvent::ToolFinished { .. }))
                .count(),
            1,
            "{events:?}"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            RuntimeEvent::ToolFinished { output, .. } if output.contains("alpha")
        )));
    }

    #[test]
    fn a_republished_text_part_is_never_counted_twice() {
        let (answer, _, events) = run(&[
            json!({"type":"text","part":{"id":"p1","type":"text","text":"He"}}),
            json!({"type":"text","part":{"id":"p1","type":"text","text":"Hello"}}),
            json!({"type":"text","part":{"id":"p1","type":"text","text":"Hello there"}}),
            json!({"type":"text","part":{"id":"p1","type":"text","text":"Hello there"}}),
        ]);
        assert_eq!(answer, "Hello there");
        assert_eq!(
            events,
            vec![
                RuntimeEvent::TextDelta { text: "He".into() },
                RuntimeEvent::TextDelta { text: "llo".into() },
                RuntimeEvent::TextDelta {
                    text: " there".into()
                },
            ]
        );
    }

    #[test]
    fn reasoning_stays_off_the_answer() {
        let (answer, _, events) = run(&[
            json!({"type":"reasoning","part":{"id":"r1","type":"reasoning","text":"think"}}),
            json!({"type":"text","part":{"id":"t1","type":"text","text":"answer"}}),
        ]);
        assert_eq!(answer, "answer");
        assert!(events.contains(&RuntimeEvent::ReasoningDelta {
            text: "think".into()
        }));
    }

    #[test]
    fn an_exhausted_output_budget_is_token_insufficient() {
        let (_, error, _) = run(&[json!({
            "type":"error","error":{"name":"MessageOutputLengthError","data":{"message":"max tokens"}}
        })]);
        assert!(is_token_insufficient(&error.unwrap()));
        let (_, error, _) =
            run(&[json!({"type":"step_finish","part":{"type":"step-finish","reason":"length"}})]);
        assert!(is_token_insufficient(&error.unwrap()));
    }

    #[test]
    fn an_exhausted_account_balance_is_not_disguised_as_a_token_limit() {
        // Rotating Agent on a quota failure would hide the real cause behind a relay
        // handover, so this must stay an ordinary error.
        let (_, error, _) = run(&[json!({
            "type":"error","timestamp":1,"sessionID":"s",
            "error":{"type":"provider.quota","message":"Upstream request failed: Insufficient account funds","status":402}
        })]);
        let error = error.unwrap();
        assert!(!is_token_insufficient(&error), "{error}");
        assert!(error.contains("Insufficient account funds"));
    }

    #[test]
    fn execute_announces_real_code_when_pending_arguments_arrive() {
        let input = json!({"code":"const r = await tools.crabot_tool_find({ target: \"tool\", query: \"shell\" });"});
        let mut parser = EventParser::default();
        let mut events = vec![];
        parser.consume(&json!({"type":"tool_use","part":{"id":"x","tool":"execute","state":{"status":"pending","input":{}}}}), &mut |e| events.push(e));
        assert!(events.is_empty());
        for status in ["running", "running", "completed"] {
            parser.consume(&json!({"type":"tool_use","part":{"id":"x","tool":"execute","state":{"status":status,"input":input,"output":"ok"}}}), &mut |e| events.push(e));
        }
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], RuntimeEvent::ToolStarted { name, arguments, .. } if name=="execute" && arguments==&input)
        );
        assert!(matches!(&events[1], RuntimeEvent::ToolFinished { .. }));
    }

    #[test]
    fn a_plain_failure_is_not_mistaken_for_a_token_problem() {
        let (_, error, _) = run(&[json!({
            "type":"error","error":{"type":"provider.no-route","message":"Model unavailable: opencode/nope"}
        })]);
        let error = error.unwrap();
        assert!(!is_token_insufficient(&error));
        assert!(error.contains("no-route") || error.contains("unavailable"));
    }

    #[test]
    fn unknown_and_log_lines_never_break_the_answer() {
        let (answer, error, _) = run(&[
            json!({"type":"step_start","part":{"id":"a","type":"step-start"}}),
            json!({"type":"something_new_v2","part":{"id":"b"}}),
            json!({"type":"text","part":{"id":"c","type":"text","text":"fine"}}),
        ]);
        assert_eq!(answer, "fine");
        assert!(error.is_none());
    }
}
