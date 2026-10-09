use crate::*;
use axum::http::{HeaderValue, Method};
use axum::response::IntoResponse;
use tower_http::cors::CorsLayer;

pub async fn index() -> Result<impl IntoResponse, StatusCode> {
    static_file("index.html").await
}
pub async fn asset(Path(path): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    static_file(&format!("assets/{path}")).await
}
async fn static_file(path: &str) -> Result<axum::response::Response, StatusCode> {
    use std::path::{Component, PathBuf};
    let relative = PathBuf::from(path);
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let mime = match relative.extension().and_then(|s| s.to_str()) {
        Some("html") if path == "index.html" => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        _ => return Err(StatusCode::NOT_FOUND),
    };
    let root =
        PathBuf::from(std::env::var("WEB_CONFIG_DIR").unwrap_or_else(|_| "apps/web/dist".into()));
    let root = tokio::fs::canonicalize(root)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let file = tokio::fs::canonicalize(root.join(relative))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if !file.starts_with(root) {
        return Err(StatusCode::NOT_FOUND);
    }
    let data = tokio::fs::read(file)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((
        [
            ("content-type", mime),
            ("x-content-type-options", "nosniff"),
            ("cache-control", "no-cache"),
        ],
        data,
    )
        .into_response())
}

pub fn cors() -> CorsLayer {
    let origins = std::env::var("WEB_CONFIG_ORIGINS").unwrap_or_else(|_| String::new());
    cors_for(&origins)
}

fn cors_for(origins: &str) -> CorsLayer {
    let origins: Vec<HeaderValue> = origins
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|origin| {
            let origin = origin.trim();
            let url = Url::parse(origin)
                .expect("WEB_CONFIG_ORIGINS must contain explicit HTTP(S) origins");
            assert!(
                matches!(url.scheme(), "http" | "https")
                    && url.origin().ascii_serialization() == origin,
                "WebConfig origin must have no path, credentials or wildcard"
            );
            origin.parse().expect("invalid WebConfig origin")
        })
        .collect();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            "content-type".parse().unwrap(),
            "authorization".parse().unwrap(),
        ])
}

pub fn proxy_mode() -> bool {
    match std::env::var("AGENT_MODE")
        .unwrap_or_else(|_| "agent".into())
        .as_str()
    {
        "agent" => false,
        "proxy" => true,
        _ => panic!("AGENT_MODE must be agent or proxy"),
    }
}

pub async fn profile(State(state): State<AppState>) -> Json<Value> {
    let proxy = proxy_mode();
    Json(
        json!({"id":state.node_id,"name":std::env::var("AGENT_NAME").unwrap_or_else(|_| if proxy {"ProxyAgent".into()} else {"My Crabot".into()}),"mode":if proxy {"proxy"} else {"agent"},"architecture":"crabot-webconfig","storage":"local-json","capabilities":if proxy {vec!["descendant-control","versioned-subgroups","execution-snapshots","sessions","history"]} else {vec!["local-execution","descendant-control","versioned-subgroups","execution-snapshots","sessions","history"]},"runtime_provider":std::env::var("AGENT_PROVIDER").unwrap_or_else(|_| "crabot".into())}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn static_files_reject_parent_and_non_assets() {
        for path in [
            "../index.html",
            "/index.html",
            "assets/../../index.html",
            ".env",
            "Cargo.toml",
        ] {
            assert!(static_file(path).await.is_err());
        }
    }
    #[test]
    fn explicit_origins_only() {
        let _ = cors_for("https://config.example.com,http://localhost:5173");
        for bad in [
            "*",
            "https://config.example.com/path",
            "https://user:password@config.example.com",
        ] {
            assert!(std::panic::catch_unwind(|| cors_for(bad)).is_err());
        }
    }
}
