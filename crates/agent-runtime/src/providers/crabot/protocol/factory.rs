use super::super::config::ModelApi;
use super::{
    anthropic::AnthropicProtocol, chat::ChatProtocol, contract::ModelProtocol,
    responses::ResponsesProtocol,
};
pub(in super::super) struct ProtocolFactory;
impl ProtocolFactory {
    pub(in super::super) fn create(api: ModelApi) -> Box<dyn ModelProtocol> {
        match api {
            ModelApi::Chat => Box::new(ChatProtocol),
            ModelApi::Responses => Box::new(ResponsesProtocol),
            ModelApi::Anthropic => Box::new(AnthropicProtocol::new()),
        }
    }
}
