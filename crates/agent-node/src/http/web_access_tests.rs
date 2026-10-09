use super::*;

const TOKEN: &str = "example-random-access-token-for-tests-1234";
fn headers(authorization: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, "192.0.2.1:8787".parse().unwrap());
    if let Some(value) = authorization {
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
    }
    headers
}
fn peer() -> SocketAddr {
    "192.0.2.2:12345".parse().unwrap()
}

#[test]
fn public_binding_requires_strong_startup_credential() {
    let local = WebAccess::new(None, "").unwrap();
    assert!(local.validate_bind("127.0.0.1:0".parse().unwrap()).is_ok());
    for address in ["0.0.0.0:8787", "[::]:8787", "192.0.2.1:8787"] {
        assert!(local.validate_bind(address.parse().unwrap()).is_err());
        assert!(
            WebAccess::new(Some(TOKEN), "")
                .unwrap()
                .validate_bind(address.parse().unwrap())
                .is_ok()
        );
    }
    for token in [
        "short",
        "                            ",
        "a-token-with-newline-123456\n",
    ] {
        assert!(WebAccess::new(Some(token), "").is_err());
    }
    assert!(WebAccess::new(Some(&"a".repeat(513)), "").is_err());
}

#[test]
fn default_access_stays_loopback_only() {
    let access = WebAccess::new(None, "").unwrap();
    let mut headers = headers(None);
    assert_eq!(access.check(&headers, peer()), Err(StatusCode::FORBIDDEN));
    let local = "127.0.0.1:1000".parse().unwrap();
    assert_eq!(access.check(&headers, local), Err(StatusCode::FORBIDDEN));
    headers.insert(header::HOST, "localhost:8787".parse().unwrap());
    assert!(access.check(&headers, local).is_ok());
}

#[test]
fn basic_and_bearer_authenticate_remote_requests() {
    let access = WebAccess::new(Some(TOKEN), "").unwrap();
    for authorization in [
        format!("Bearer {TOKEN}"),
        format!("Basic {}", STANDARD.encode(format!("crabot:{TOKEN}"))),
    ] {
        assert!(access.check(&headers(Some(&authorization)), peer()).is_ok());
    }
    for authorization in [
        "Bearer wrong".into(),
        "Basic invalid!".into(),
        format!("Basic {}", STANDARD.encode(format!("other:{TOKEN}"))),
        "Digest token".into(),
    ] {
        assert_eq!(
            access.check(&headers(Some(&authorization)), peer()),
            Err(StatusCode::UNAUTHORIZED)
        );
    }
    assert_eq!(
        access.check(&headers(None), peer()),
        Err(StatusCode::UNAUTHORIZED)
    );
    // Configuring a token also protects localhost; no proxy/loopback bypass.
    assert_eq!(
        access.check(&headers(None), "127.0.0.1:1".parse().unwrap()),
        Err(StatusCode::UNAUTHORIZED)
    );
}

#[test]
fn browser_auth_does_not_bypass_origin_checks() {
    let access = WebAccess::new(Some(TOKEN), "https://config.example").unwrap();
    let mut headers = headers(Some(&format!(
        "Basic {}",
        STANDARD.encode(format!("crabot:{TOKEN}"))
    )));
    for origin in ["http://192.0.2.1:8787", "https://config.example"] {
        headers.insert(header::ORIGIN, origin.parse().unwrap());
        assert!(access.check(&headers, peer()).is_ok());
    }
    for origin in ["https://evil.example", "null", "http://192.0.2.1:9999"] {
        headers.insert(header::ORIGIN, origin.parse().unwrap());
        assert_eq!(access.check(&headers, peer()), Err(StatusCode::FORBIDDEN));
    }
    headers.remove(header::ORIGIN);
    headers.insert("sec-fetch-site", "cross-site".parse().unwrap());
    assert_eq!(access.check(&headers, peer()), Err(StatusCode::FORBIDDEN));
}
