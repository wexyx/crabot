use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    prerelease: bool,
    draft: bool,
}
fn version(value: &str) -> Option<(u64, u64, u64, u64)> {
    let mut parts = value.strip_prefix('v')?.split('-').next()?.split('.');
    let result = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next().map(str::parse).transpose().ok()?.unwrap_or(0),
    );
    parts.next().is_none().then_some(result)
}
pub(super) fn newer(current: &str, latest: &str) -> bool {
    match (version(current), version(latest)) {
        (Some(a), Some(b)) => b > a || (a == b && current.contains('-')),
        _ => false,
    }
}
pub(super) async fn check() -> Result<Option<String>, String> {
    let repo = std::env::var("CRABOT_REPOSITORY").unwrap_or_else(|_| "wexyx/crabot".into());
    if repo.split('/').count() != 2
        || repo
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || !repo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err("invalid release repository".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Crabot-update-check")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let release: Release = response.json().await.map_err(|e| e.to_string())?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    // Only display validated version text from the remote response.
    if version(&release.tag_name).is_none()
        || release.tag_name.contains('-')
        || release.tag_name.len() > 64
    {
        return Err("invalid release tag".into());
    }
    Ok(newer(crate::app::version::DISPLAY, &release.tag_name).then_some(release.tag_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_numeric_versions_and_development_builds() {
        assert!(newer("v1.9.0", "v1.10.0"));
        assert!(newer("v1.2.3-dev", "v1.2.3"));
        assert!(!newer("v1.2.3", "v1.2.3"));
        assert!(!newer("v2.0.0", "v1.9.0"));
        assert!(!newer("v1.0.0", "v9.0.0\n"));
    }

    #[test]
    fn compares_four_part_patch_releases() {
        assert!(newer("v0.1.4", "v0.1.4.1"));
        assert!(newer("v0.1.4.1", "v0.1.4.2"));
        assert!(newer("v0.1.4.9", "v0.1.4.10"));
        assert!(newer("v0.1.4.1", "v0.1.5"));
        assert!(newer("v0.1.4.1-dev", "v0.1.4.1"));
        assert!(!newer("v0.1.4.1", "v0.1.4"));
        assert!(!newer("v0.1.4.1", "v0.1.4.1"));
        assert!(!newer("v0.1.4", "v0.1.4.0"));
        assert!(version("v0.1.4.1.2").is_none());
        assert!(version("v0.1.4.x").is_none());
        assert!(version("v0.1.4.1\n").is_none());
    }
}
