use super::client::ModelClient;
use crate::{
    RuntimeEvent,
    tools::{ToolRegistry, ToolSession},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
/// Per-task conversation and tool authorization state.
pub(super) struct Run<'a> {
    client: &'a ModelClient,
    tools: &'a ToolRegistry,
    history: Vec<Value>,
    session: ToolSession,
}
impl<'a> Run<'a> {
    pub(super) fn new(client: &'a ModelClient, tools: &'a ToolRegistry, prompt: &str) -> Self {
        Self {
            client,
            tools,
            history: vec![json!({"role":"user","content":prompt})],
            session: ToolSession::default(),
        }
    }
    pub(super) async fn execute(
        mut self,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<String, String> {
        loop {
            tokio::task::yield_now().await;
            for content in crate::context::Guidance::drain() {
                self.history.push(json!({"role":"user","content":content}));
            }
            // Projected per turn so a tool revealed by `find` is offered by the
            // next request; the run executes against the full registry regardless.
            let exposed = self.tools.exposed(&self.session);
            self.client
                .prepare_context(&mut self.history, &exposed, events)
                .await?;
            let turn = self
                .client
                .next_turn(&self.history, &exposed, events)
                .await?;
            if turn.calls().is_empty() {
                if crate::context::Guidance::pending() {
                    self.history
                        .push(json!({"role":"assistant","content":turn.text()}));
                    continue;
                }
                return Ok(turn.text().into());
            }
            let mut results = BTreeMap::new();
            for &index in turn.calls().keys() {
                let (id, name) = turn.call(index)?;
                if crate::context::Guidance::pending() {
                    results.insert(index,"Tool not started: new user guidance arrived. Reassess the plan before executing.".into());
                    continue;
                }
                let args: Value = serde_json::from_str(turn.arguments(index))
                    .map_err(|_| "invalid tool arguments JSON")?;
                events(RuntimeEvent::ToolStarted {
                    id: id.into(),
                    name: name.into(),
                    arguments: args.clone(),
                });
                let result = self
                    .tools
                    .execute(name, &args, &mut self.session)
                    .await
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_else(|e| json!({"error":e}).to_string());
                events(RuntimeEvent::ToolFinished {
                    id: id.into(),
                    output: result.clone(),
                });
                results.insert(index, result);
            }
            let batch_start = self.history.len();
            self.client
                .append_history(&mut self.history, &turn, &results);
            // Host-driven compaction: the tool call was only a trigger. The host
            // compresses the window through a no-tools model request, replaces older
            // rounds with the summary, and persists the summary (best-effort).
            if self.session.take_compact() {
                match self.client.compact(&self.history).await {
                    Ok(summary) => {
                        crate::context::apply_summary(&mut self.history, batch_start, &summary);
                        let persisted =
                            crate::context::HistoryAccess::write_summary(&summary).await;
                        let note = match persisted {
                            Ok(_) => String::new(),
                            Err(error) => format!("\nSummary not persisted: {error}"),
                        };
                        events(RuntimeEvent::ContextCheckpoint {
                            content: format!("Context compacted by Agent: {summary}{note}"),
                        });
                    }
                    Err(error) => events(RuntimeEvent::ContextCheckpoint {
                        content: format!("Compaction failed: {error}"),
                    }),
                }
            } else if let Some(summary) = self.session.take_summary() {
                crate::context::apply_summary(&mut self.history, batch_start, &summary);
                events(RuntimeEvent::ContextCheckpoint {
                    content: format!("Context compacted by Agent: {summary}"),
                });
            }
        }
    }
}
