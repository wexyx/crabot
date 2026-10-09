use crate::storage::Store;
pub(crate) async fn preferred(store: &Store) -> String {
    let configured = std::env::var("BIND_ADDR").ok();
    let defaults = std::env::var("CRABOT_CONFIG_DEFAULT_KEYS").unwrap_or_default();
    let explicit = std::env::var("CRABOT_WEB_PORT_EXPLICIT").ok().as_deref() == Some("1")
        || !defaults.split_whitespace().any(|key| key == "BIND_ADDR");
    if explicit {
        if let Some(address) = configured.as_ref() {
            return address.clone();
        }
    }
    store
        .get("instance_settings", "web")
        .await
        .and_then(|r| r["address"].as_str().map(str::to_owned))
        .or(configured)
        .unwrap_or_else(|| "127.0.0.1:8787".into())
}
