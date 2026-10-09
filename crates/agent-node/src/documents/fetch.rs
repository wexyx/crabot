use futures_util::StreamExt;
use std::net::{IpAddr, Ipv4Addr};
use url::Url;
pub(super) const MAX_BYTES: usize = 20 * 1024 * 1024;

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !ip.is_private()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_broadcast()
        && !ip.is_documentation()
        && !ip.is_unspecified()
        && a != 0
        && a < 224
        && !(a == 100 && (64..=127).contains(&b))
        && !(a == 198 && (b == 18 || b == 19))
        && !(a == 192 && b == 0 && c == 0)
}
fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        // Only globally routable unicast; reject transition/translation prefixes, too.
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && !(s[0] == 0x3fff && s[1] <= 0x0fff)
        }
    }
}
pub(super) async fn fetch(
    raw: &str,
    ignore: &[String],
) -> Result<(Vec<u8>, String, String), String> {
    tokio::time::timeout(std::time::Duration::from_secs(60), fetch_inner(raw, ignore))
        .await
        .map_err(|_| "Document download timed out".to_string())?
}
async fn fetch_inner(raw: &str, ignore: &[String]) -> Result<(Vec<u8>, String, String), String> {
    let mut url = Url::parse(raw).map_err(|e| e.to_string())?;
    for hop in 0..=5 {
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(url.port_or_known_default(), Some(80 | 443))
        {
            return Err("Only public HTTP(S) document URLs on ports 80/443 are allowed".into());
        }
        url = super::links::normalize(url, ignore);
        if url.fragment().is_some() {
            return Err("Hash-router pages need browser Skill extraction before saving; they cannot be fetched as static HTTP documents".into());
        }
        let host = url.host_str().ok_or("URL host missing")?;
        let addresses = tokio::net::lookup_host((
            host.trim_matches(['[', ']']),
            url.port_or_known_default().unwrap(),
        ))
        .await
        .map_err(|e| format!("Document DNS: {e}"))?
        .collect::<Vec<_>>();
        if addresses.is_empty() || addresses.iter().any(|a| !public(a.ip())) {
            return Err("Document URL resolves to a private/reserved address".into());
        }
        // Pin the validated resolution, disable inherited proxies and automatic redirects.
        // Each redirect is resolved and checked independently (including DNS rebinding).
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &addresses)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get(url.clone())
            .header(reqwest::header::USER_AGENT, "Crabot-Documents/1.0")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if response.status().is_redirection() {
            if hop == 5 {
                return Err("Too many document redirects".into());
            }
            url = url
                .join(
                    response
                        .headers()
                        .get(reqwest::header::LOCATION)
                        .ok_or("Redirect has no Location")?
                        .to_str()
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            continue;
        }
        let response = response.error_for_status().map_err(|e| e.to_string())?;
        if response
            .content_length()
            .is_some_and(|n| n > MAX_BYTES as u64)
        {
            return Err("Document exceeds 20 MiB".into());
        }
        let kind = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let mut stream = response.bytes_stream();
        let mut bytes = vec![];
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            if bytes.len() + chunk.len() > MAX_BYTES {
                return Err("Document exceeds 20 MiB".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok((bytes, kind, url.to_string()));
    }
    unreachable!()
}
#[cfg(test)]
mod tests {
    #[test]
    fn rejects_special_ranges() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "198.18.1.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "2001:db8::1",
            "2002:7f00:1::",
        ] {
            assert!(!super::public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
            assert!(super::public(ip.parse().unwrap()));
        }
    }
}
