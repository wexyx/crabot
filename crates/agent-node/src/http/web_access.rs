use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use subtle::ConstantTimeEq;

/// Immutable startup policy. Credentials never enter model-editable state.
#[derive(Clone)]
pub(crate) struct WebAccess {
    token_hash: Option<[u8; 32]>,
    origins: Vec<String>,
}

impl WebAccess {
    pub(crate) fn new(token: Option<&str>, origins: &str) -> Result<Self, String> {
        let token = token.filter(|value| !value.is_empty());
        if token.is_some_and(|value| {
            !(24..=512).contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_graphic())
        }) {
            return Err("WEB_ACCESS_TOKEN must contain 24..512 visible ASCII characters; generate it with openssl rand -hex 32".into());
        }
        Ok(Self {
            token_hash: token.map(|value| Sha256::digest(value.as_bytes()).into()),
            origins: origins
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }

    pub(crate) fn from_env() -> Result<Self, String> {
        Self::new(
            std::env::var("WEB_ACCESS_TOKEN").ok().as_deref(),
            &std::env::var("WEB_CONFIG_ORIGINS").unwrap_or_default(),
        )
    }

    pub(crate) fn validate_bind(&self, address: SocketAddr) -> Result<(), String> {
        if !address.ip().is_loopback() && self.token_hash.is_none() {
            return Err("Remote Web requires WEB_ACCESS_TOKEN (at least 24 characters). Set it in the instance .agent.env before binding a non-loopback address.".into());
        }
        Ok(())
    }

    fn check(&self, headers: &HeaderMap, peer: SocketAddr) -> Result<(), StatusCode> {
        let Some(expected) = self.token_hash else {
            if !peer.ip().is_loopback() {
                return Err(StatusCode::FORBIDDEN);
            }
            return super::local_access::check(headers);
        };
        // Browser-cached Basic credentials must not authorize cross-site requests.
        if let Some(origin) = headers.get(header::ORIGIN) {
            let origin = origin.to_str().map_err(|_| StatusCode::FORBIDDEN)?;
            let uri = origin
                .parse::<axum::http::Uri>()
                .map_err(|_| StatusCode::FORBIDDEN)?;
            let same_host = matches!(uri.scheme_str(), Some("http" | "https"))
                && uri.authority().is_some_and(|authority| {
                    headers.get(header::HOST).and_then(|v| v.to_str().ok())
                        == Some(authority.as_str())
                });
            if !same_host && !self.origins.iter().any(|allowed| allowed == origin) {
                return Err(StatusCode::FORBIDDEN);
            }
        } else if headers
            .get("sec-fetch-site")
            .is_some_and(|v| v == "cross-site")
        {
            return Err(StatusCode::FORBIDDEN);
        }
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let (scheme, value) = authorization
            .split_once(' ')
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let token = if scheme.eq_ignore_ascii_case("Bearer") {
            value.to_owned()
        } else if scheme.eq_ignore_ascii_case("Basic") {
            let decoded = STANDARD
                .decode(value)
                .map_err(|_| StatusCode::UNAUTHORIZED)?;
            let decoded = String::from_utf8(decoded).map_err(|_| StatusCode::UNAUTHORIZED)?;
            let (user, password) = decoded.split_once(':').ok_or(StatusCode::UNAUTHORIZED)?;
            if user != "crabot" {
                return Err(StatusCode::UNAUTHORIZED);
            }
            password.to_owned()
        } else {
            return Err(StatusCode::UNAUTHORIZED);
        };
        let actual: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        if bool::from(actual.ct_eq(&expected)) {
            Ok(())
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

pub(crate) async fn guard(
    State(access): State<WebAccess>,
    request: Request,
    next: Next,
) -> Response {
    let result = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .ok_or(StatusCode::FORBIDDEN)
        .and_then(|peer| access.check(request.headers(), peer.0));
    match result {
        Ok(()) => next.run(request).await,
        Err(StatusCode::UNAUTHORIZED) => (
            StatusCode::UNAUTHORIZED,
            [
                (
                    header::WWW_AUTHENTICATE,
                    "Basic realm=\"Crabot\", charset=\"UTF-8\"",
                ),
                (header::CACHE_CONTROL, "no-store"),
            ],
            "Crabot requires authentication. Username: crabot; password: WEB_ACCESS_TOKEN.",
        )
            .into_response(),
        Err(status) => status.into_response(),
    }
}

#[cfg(test)]
#[path = "web_access_tests.rs"]
mod tests;
