mod a2a;
mod app;
mod capabilities;
mod cli;
mod configuration;
mod core;
mod documents;
mod http;
mod management;
mod prelude;
mod storage;
#[cfg(test)]
mod tests;

pub(crate) use a2a::{control, enrollment, node, peer_handlers};
pub use app::entry::run;
pub(crate) use auth::*;
pub(crate) use core::{conversation, events as event_service, local, policies, skills, state};
pub(crate) use event_service::*;
pub(crate) use http::events::sse;
#[cfg(test)]
pub(crate) use http::routes::router;
pub(crate) use http::{
    assets as web_config, auth, repl as repl_http, server as web_server,
    sessions as session_handlers, workspace as workspace_handlers,
};
pub(crate) use peer_handlers::*;
pub(crate) use prelude::*;
pub(crate) use session_handlers::*;
pub(crate) use state::*;
pub(crate) use storage::policies as policy_store;
#[cfg(test)]
pub(crate) use tests::groups as crabot_tests;
