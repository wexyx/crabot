use super::config::HarnessConfig;
use super::engine::Engine;
use crate::{EventSink, RuntimeFuture, RuntimeKind};

pub(crate) struct Runtime {
    engine: Engine,
}

impl Runtime {
    pub(crate) fn new(
        config: HarnessConfig,
        tools: crate::tools::ToolRegistry,
    ) -> Result<Self, String> {
        Ok(Self {
            engine: Engine::new(config, tools)?,
        })
    }
}

impl crate::providers::Provider for Runtime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Crabot
    }
    fn handles_tools(&self) -> bool {
        true
    }
    fn execute<'a>(
        &'a self,
        prompt: &'a str,
        on_event: &'a mut EventSink<'_>,
    ) -> RuntimeFuture<'a> {
        // No catalog preamble: skills reach the model through `find`, which
        // also decides whether this task needs one at all.
        Box::pin(async move { self.engine.run(prompt, &mut |event| on_event(event)).await })
    }
}
