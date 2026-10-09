use super::{
    config::HarnessConfig,
    model_error::model_error,
    protocol::{ModelProtocol, ProtocolFactory},
    turn::{Delta, Turn},
};
use crate::{RuntimeEvent, context::SummaryPlan, tools::ToolRegistry};
use agent_protocol::SseDecoder;
use futures_util::StreamExt;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
pub(super) struct ModelClient {
    http: reqwest::Client,
    config: HarnessConfig,
    protocol: Arc<dyn ModelProtocol>,
}

#[cfg(test)]
mod prompt_tests {
    use super::*;
    #[test]
    fn prompt_files_reload_between_tasks_not_inside_a_running_task() {
        let dir = tempfile::tempdir().unwrap();
        let prompts = crate::prompts::PromptStore::new(dir.path());
        prompts.validate_all().unwrap();
        let config = HarnessConfig::from_lookup(|key| match key {
            "MODEL_PROVIDER" => Some("ollama".into()),
            "MODEL_NAME" => Some("fixture".into()),
            "AGENT_WORKDIR" => Some(dir.path().to_string_lossy().into_owned()),
            _ => None,
        })
        .unwrap();
        let client = ModelClient::new(config.clone()).unwrap();
        let first = client.with_prompts(&prompts).unwrap();
        let original = prompts.read("agent").unwrap();
        prompts
            .save("agent", "custom second task", &original)
            .unwrap();
        assert_eq!(first.config.system_prompt, original);
        assert_eq!(
            client.with_prompts(&prompts).unwrap().config.system_prompt,
            "custom second task"
        );
        let mut explicit = config;
        explicit.system_prompt = "agent-specific instructions".into();
        assert_eq!(
            ModelClient::new(explicit)
                .unwrap()
                .with_prompts(&prompts)
                .unwrap()
                .config
                .system_prompt,
            "agent-specific instructions"
        );
    }
}
impl ModelClient {
    /// Snapshot editable instance instructions once per task, keeping HTTP pooling.
    pub(super) fn for_run(&self) -> Result<Self, String> {
        self.with_prompts(&crate::prompts::PromptStore::instance())
    }
    fn with_prompts(&self, prompts: &crate::prompts::PromptStore) -> Result<Self, String> {
        let mut config = self.config.clone();
        if config.system_prompt.trim() == super::prompt::default_system_prompt() {
            config.system_prompt = prompts.read("agent")?;
        }
        Ok(Self {
            http: self.http.clone(),
            protocol: self.protocol.clone(),
            config,
        })
    }
    pub(super) fn new(config: HarnessConfig) -> Result<Self, String> {
        let protocol = ProtocolFactory::create(config.api).into();
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(180));
        if [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
        ]
        .iter()
        .any(|key| config.environment.get(key).is_some())
        {
            builder = builder.no_proxy();
            let lookup = |key: &str| {
                config
                    .environment
                    .get(key)
                    .or_else(|| config.environment.get(&key.to_ascii_lowercase()))
                    .cloned()
                    .or_else(|| std::env::var(key).ok())
                    .or_else(|| std::env::var(key.to_ascii_lowercase()).ok())
            };
            let bypass = lookup("NO_PROXY").and_then(|s| reqwest::NoProxy::from_string(&s));
            for key in ["HTTPS_PROXY", "HTTP_PROXY", "ALL_PROXY"] {
                if let Some(value) = lookup(key).filter(|v| !v.is_empty()) {
                    let proxy = match key {
                        "HTTPS_PROXY" => reqwest::Proxy::https(&value),
                        "HTTP_PROXY" => reqwest::Proxy::http(&value),
                        _ => reqwest::Proxy::all(&value),
                    }
                    .map_err(|_| "Agent 代理地址无效")?;
                    builder = builder.proxy(proxy.no_proxy(bypass.clone()));
                }
            }
        }
        let http = builder.build().map_err(|e| e.to_string())?;
        Ok(Self {
            http,
            config,
            protocol,
        })
    }
    pub(super) async fn prepare_context(
        &self,
        history: &mut Vec<Value>,
        tools: &ToolRegistry,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<(), String> {
        let image_reserve =
            crate::attachments::PreparedAttachments::images(|images| images.len() * 8192);
        let limit = self.config.input_limit()?.checked_sub(image_reserve).ok_or("CONTEXT_LIMIT: images exceed context length; attach fewer images or increase context length")?;
        let size = |rows: &[Value]| -> Result<usize, String> {
            let request = self
                .protocol
                .request(&self.http, &self.config, rows, tools)
                .build()
                .map_err(|e| e.to_string())?;
            Ok(request
                .body()
                .and_then(|b| b.as_bytes())
                .map_or(0, |b| b.len()))
        };
        let before = size(history)?;
        if before <= limit {
            return Ok(());
        }
        let fixed = size(&[])?;
        let mut available = limit
            .checked_sub(fixed + 1024)
            .ok_or("CONTEXT_LIMIT: system prompt and tool schemas exceed input budget")?;
        let sources = crate::context::HistoryAccess::manifest();
        if let Some(plan) = self
            .config
            .context
            .summary_plan(history, available.saturating_sub(sources.len()))?
        {
            events(RuntimeEvent::ContextCheckpoint {
                content: "正在智能压缩上下文…".into(),
            });
            let summary = self.summarize(&plan, limit, events).await?;
            let candidate = plan.finish(&summary, &sources);
            let after = size(&candidate)?;
            if after > limit {
                return Err("CONTEXT_LIMIT: 智能摘要仍超过上下文长度，原始上下文未修改".into());
            }
            if crate::context::HistoryAccess::is_bound() {
                crate::context::HistoryAccess::write_summary(&plan.archive(&summary)).await?;
            }
            *history = candidate;
            events(RuntimeEvent::ContextCheckpoint {
                content: format!(
                    "Context compacted: 智能压缩 {before} -> {after}; summary={summary}; {sources}"
                ),
            });
            return Ok(());
        }
        // JSON escaping can expand excerpts. Measure the actual request before sending.
        for _ in 0..6 {
            let candidate = self.config.context.compact(history, available)?;
            let after = size(&candidate)?;
            if after <= limit {
                *history = candidate;
                events(RuntimeEvent::ContextCheckpoint {
                    content: format!(
                        "Context compacted: estimated input bytes {before} -> {after}; original logs retained. Lossy local excerpts, not a model-generated summary."
                    ),
                });
                return Ok(());
            }
            available = available * 3 / 4;
        }
        Err("CONTEXT_LIMIT: unable to fit request; use /new or increase context budget".into())
    }
    /// Compress a window through a no-tools model request, for the tool-triggered
    /// compaction path. The budget mirrors the input-limit path, so the summary
    /// always fits this run's own context length.
    pub(super) async fn compact(&self, history: &[Value]) -> Result<String, String> {
        let limit = self.config.input_limit()?;
        let size = |rows: &[Value]| -> Result<usize, String> {
            let request = self
                .protocol
                .request(&self.http, &self.config, rows, &ToolRegistry::new())
                .build()
                .map_err(|e| e.to_string())?;
            Ok(request
                .body()
                .and_then(|b| b.as_bytes())
                .map_or(0, |b| b.len()))
        };
        let fixed = size(&[])?;
        let available = limit
            .checked_sub(fixed + 1024)
            .ok_or("CONTEXT_LIMIT: 固定指令过长，无法压缩上下文")?;
        let sources = crate::context::HistoryAccess::manifest();
        let plan =
            crate::context::SummaryPlan::new(history, available.saturating_sub(sources.len()))?;
        let summary = self.summarize(&plan, limit, &mut |_| {}).await?;
        Ok(plan.archive(&summary))
    }

    /// Run the chunked model-driven compaction for a plan.
    ///
    /// Shared by the input-limit path (`prepare_context`) and the tool-triggered
    /// path (`Run` after `compact`): both need the same no-tools request,
    /// chunked to fit `limit`, with the same structural validation of the answer.
    pub(super) async fn summarize(
        &self,
        plan: &SummaryPlan,
        limit: usize,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<String, String> {
        let size = |rows: &[Value]| -> Result<usize, String> {
            let request = self
                .protocol
                .request(&self.http, &self.config, rows, &ToolRegistry::new())
                .build()
                .map_err(|e| e.to_string())?;
            Ok(request
                .body()
                .and_then(|b| b.as_bytes())
                .map_or(0, |b| b.len()))
        };
        let mut summary = String::new();
        let mut remaining = plan.older();
        while !remaining.is_empty() {
            let mut chunk_limit = (limit / 2).max(256);
            let (chunk, input) = loop {
                let chunk = crate::context::SummaryPlan::chunk(remaining, chunk_limit);
                let input = vec![serde_json::json!(
                    {"role":"user","content":plan.summary_request(&summary,chunk)}
                )];
                if size(&input)?.saturating_add(512) <= limit {
                    break (chunk, input);
                }
                if chunk_limit <= 256 {
                    return Err("CONTEXT_LIMIT: 摘要请求无法容纳于上下文长度".into());
                }
                chunk_limit /= 2;
            };
            let mut last_valid_oversize = None;
            for attempt in 0..3 {
                let mut request = input.clone();
                if attempt > 0 {
                    request[0]["content"] = serde_json::json!(format!(
                        "{}\nRETRY: the previous output was invalid or too long. Return only the required JSON arrays, no tools or markdown. Prefer fewer short entries, prioritize constraints and pending work. Target at most {} UTF-8 bytes, including JSON escaping. Do not answer the historical task.",
                        input[0]["content"].as_str().unwrap(),
                        plan.summary_limit() / 2
                    ));
                    events(RuntimeEvent::ContextCheckpoint {
                        content: format!("摘要需要精简，正在重试（{attempt}/2）…"),
                    });
                }
                if size(&request)? > limit {
                    return Err(
                        "CONTEXT_LIMIT: 摘要重试请求超过上下文长度，原始上下文未修改".into(),
                    );
                }
                let turn = self
                    .request_turn(&request, &ToolRegistry::new(), &mut |_| {})
                    .await?;
                let validation = if turn.calls().is_empty() {
                    plan.validate(turn.text())
                } else {
                    Err("智能压缩期间模型请求了工具，摘要未应用".into())
                };
                match validation {
                    Ok(valid) => {
                        summary = valid;
                        break;
                    }
                    Err(error) => {
                        if turn.calls().is_empty() {
                            if let Ok(fitted) = plan.fit_summary(turn.text()) {
                                last_valid_oversize = Some(fitted);
                            }
                        }
                        if attempt == 2 {
                            if let Some(fitted) = last_valid_oversize.take() {
                                summary = fitted;
                                events(RuntimeEvent::ContextCheckpoint{content:"摘要仍偏长，已保留分类要点并标记省略；原始记录可回查，继续任务。".into()});
                            } else {
                                return Err(format!("智能压缩重试后仍失败：{error}"));
                            }
                        }
                    }
                }
            }
            remaining = &remaining[chunk.len()..];
        }
        if summary.is_empty() {
            summary = plan.validate(
                r#"{"goals":[],"constraints":[],"decisions":[],"completed":[],"pending":[],"risks":[],"references":[]}"#,
            )?;
        }
        Ok(summary)
    }

    pub(super) async fn next_turn(
        &self,
        history: &[Value],
        tools: &ToolRegistry,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<Turn, String> {
        let history = self.protocol.with_images(history);
        self.request_turn(&history, tools, events).await
    }
    async fn request_turn(
        &self,
        history: &[Value],
        tools: &ToolRegistry,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<Turn, String> {
        let response = self
            .protocol
            .prepare_request(&self.http, &self.config, history, tools)
            .await?
            .send()
            .await
            .map_err(|_| "model connection failed")?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(model_error(&format!(
                "HTTP {status}: {}",
                text.chars().take(2000).collect::<String>()
            )));
        }
        let mut stream = response.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut turn = Turn::new();
        while let Some(chunk) = stream.next().await {
            for value in decoder.push_json(&chunk.map_err(|_| "model stream interrupted")?)? {
                if value.get("error").is_some()
                    || value["type"] == "error"
                    || value["type"] == "response.failed"
                {
                    return Err(model_error(&value.to_string()));
                }
                // One sink, tagged per channel: the answer and the deliberation are
                // distinct events, and presentation decides what is folded away.
                self.protocol
                    .consume(&mut turn, value, &mut |delta| match delta {
                        Delta::Answer(text) => events(RuntimeEvent::TextDelta { text }),
                        Delta::Reasoning(text) => events(RuntimeEvent::ReasoningDelta { text }),
                    })?;
            }
            // Protocol completion, not TCP EOF, ends a turn. Some SSE servers keep
            // the connection open after finish_reason / message_delta / response.completed.
            if turn.is_complete() {
                break;
            }
        }
        turn.validate()?;
        Ok(turn)
    }
    pub(super) fn append_history(
        &self,
        history: &mut Vec<Value>,
        turn: &Turn,
        results: &BTreeMap<usize, String>,
    ) {
        self.protocol.append_history(history, turn, results);
    }
}
