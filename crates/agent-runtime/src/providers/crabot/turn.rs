use serde_json::Value;

/// One streamed fragment, tagged by channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Delta {
    /// Part of the reply shown as the answer.
    Answer(String),
    /// Deliberation: reported for folded progress, never part of the answer.
    Reasoning(String),
}
use std::collections::BTreeMap;
/// One model response, independent of HTTP and tool execution.
#[derive(Default)]
pub(super) struct Turn {
    text: String,
    reasoning: String,
    calls: BTreeMap<usize, Value>,
    arguments: BTreeMap<usize, String>,
    output: Vec<Value>,
    finish: Option<String>,
}
impl Turn {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn text(&self) -> &str {
        &self.text
    }
    pub(super) fn is_complete(&self) -> bool {
        self.finish.is_some()
    }
    pub(super) fn reasoning(&self) -> &str {
        &self.reasoning
    }
    pub(super) fn calls(&self) -> &BTreeMap<usize, Value> {
        &self.calls
    }
    pub(super) fn output(&self) -> &[Value] {
        &self.output
    }
    pub(super) fn append_text(&mut self, text: &str, delta: &mut dyn FnMut(Delta)) {
        self.text.push_str(text);
        delta(Delta::Answer(text.into()));
    }
    /// Deliberation is retained for provider continuity and, unlike text, is also
    /// reported so presentation layers can fold it instead of showing it as an answer.
    pub(super) fn append_reasoning(&mut self, text: &str, delta: &mut dyn FnMut(Delta)) {
        self.reasoning.push_str(text);
        delta(Delta::Reasoning(text.into()));
    }
    pub(super) fn calls_mut(&mut self) -> &mut BTreeMap<usize, Value> {
        &mut self.calls
    }
    pub(super) fn arguments_mut(&mut self) -> &mut BTreeMap<usize, String> {
        &mut self.arguments
    }
    pub(super) fn finish(&mut self, reason: &str) {
        self.finish = Some(reason.into());
    }
    pub(super) fn set_output(&mut self, output: Vec<Value>) {
        self.output = output;
    }
    pub(super) fn arguments(&self, index: usize) -> &str {
        self.arguments
            .get(&index)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("{}")
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        let finish = self
            .finish
            .as_deref()
            .ok_or("model stream ended before completion")?;
        if ["length", "max_tokens"].contains(&finish) {
            return Err("TOKEN_INSUFFICIENT: model output token limit reached".into());
        }
        if self.calls.len() > 16 {
            return Err("model exceeded 16 tool calls per step".into());
        }
        Ok(())
    }
    pub(super) fn call(&self, index: usize) -> Result<(&str, &str), String> {
        let call = &self.calls[&index];
        Ok((
            call["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("tool call missing ID")?,
            call["name"].as_str().ok_or("tool call missing name")?,
        ))
    }
}
