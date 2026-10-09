use crate::*;
#[cfg(test)]
pub(crate) fn router(state: AppState) -> Router {
    let manager = management::Manager::new(core::Core::new(state.clone()));
    router_with_manager(state, manager).layer(axum::Extension(axum::extract::ConnectInfo(
        "127.0.0.1:1234".parse::<std::net::SocketAddr>().unwrap(),
    )))
}
pub(crate) fn router_with_manager(state: AppState, manager: Arc<management::Manager>) -> Router {
    // Human-facing transport: conversations, read models and explicit permissions only.
    let interaction = Router::new()
        .merge(crate::http::attachments::routes())
        .merge(crate::http::command_allowlist::routes(manager.clone()))
        .merge(crate::http::system_prompts::routes(manager.clone()))
        .merge(crate::http::process_sessions::routes(manager.clone()))
        .merge(crate::http::documents::routes(manager.clone()))
        .merge(crate::http::admin::routes(manager.clone()))
        .merge(crate::http::admin_configuration::routes(manager.clone()))
        .merge(crate::http::opencode::routes(manager.clone()))
        .merge(crate::http::skills::routes(manager.clone()))
        .merge(crate::http::tools::routes(manager.clone()))
        .merge(crate::http::capabilities::routes(manager.clone()))
        .merge(crate::http::agents::routes(manager.clone()))
        .merge(crate::http::connections::routes(manager.clone()))
        .merge(repl_http::routes(manager))
        .route(
            "/v1/workspace/directories",
            get(crate::http::directories::list),
        )
        .route("/v1/workspace", get(workspace_handlers::configuration))
        .route("/v1/workspace/approvals", get(workspace_handlers::pending))
        .route(
            "/v1/workspace/approvals/{id}",
            post(workspace_handlers::decide),
        )
        .route("/v1/agent", get(web_config::profile))
        .route("/v1/sessions/{id}/events", get(session_events))
        .route(
            "/v1/sessions/{id}/interrupt",
            post(session_handlers::interrupt),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            enrollment::guard,
        ));
    Router::new()
        .merge(interaction)
        .route("/", get(web_config::index))
        .route("/assets/{*path}", get(web_config::asset))
        .route("/healthz", get(|| async { Json(json!({"ok":true})) }))
        // A2A protocol is not a human management API.
        .route("/v1/client/register", post(enrollment::register))
        .route("/v1/client/connect", get(client_connect))
        .route("/v1/client/events", post(client_event))
        .route("/v1/client/control-results", post(control::reply))
        .layer(web_config::cors())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
