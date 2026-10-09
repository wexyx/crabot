use crate::storage::Store;
pub(crate) async fn preferred(store: &Store) -> String {
    let configured = std::env::var("BIND_ADDR").ok();
    // Explicit environment or .agent.env settings override a previously saved
    // listener, otherwise changing the instance file would appear ineffective.
    if let Some(address) = configured {
        return address;
    }
    store
        .get("instance_settings", "web")
        .await
        .and_then(|r| r["address"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "127.0.0.1:8787".into())
}
