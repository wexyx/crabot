use super::{client::ModelClient, config::HarnessConfig, run::Run};
use crate::{RuntimeEvent, tools::ToolRegistry};
/// Reusable dependencies; invocation state belongs to Run.
pub(super) struct Engine {
    client: ModelClient,
    tools: ToolRegistry,
}
impl Engine {
    pub(super) fn new(config: HarnessConfig, tools: ToolRegistry) -> Result<Self, String> {
        let config = config.validate()?;
        Ok(Self {
            client: ModelClient::new(config)?,
            tools,
        })
    }
    pub(super) async fn run(
        &self,
        prompt: &str,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<String, String> {
        let client = self.client.for_run()?;
        Run::new(&client, &self.tools, prompt).execute(events).await
    }
}
