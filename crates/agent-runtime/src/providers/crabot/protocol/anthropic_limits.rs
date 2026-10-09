use super::super::config::HarnessConfig;

/// Messages requires max_tokens. Discover the model's advertised ceiling rather
/// than shipping a stale model-name table or silently imposing a fixed limit.
pub(super) async fn discover(
    http: &reqwest::Client,
    config: &HarnessConfig,
) -> Result<u64, String> {
    let hint = "Anthropic 无法自动读取模型输出上限，请在 Agent 配置中填写最大输出长度（HARNESS_MAX_TOKENS）";
    let mut url =
        reqwest::Url::parse(&format!("{}/models/", config.base)).map_err(|_| hint.to_string())?;
    url.path_segments_mut()
        .map_err(|_| hint.to_string())?
        .pop_if_empty()
        .push(&config.model);
    let response = http
        .get(url)
        .header("x-api-key", &config.key)
        .header("anthropic-version", "2023-06-01")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| hint.to_string())?;
    if !response.status().is_success() {
        return Err(format!("{hint}（HTTP {}）", response.status()));
    }
    let model: serde_json::Value = response.json().await.map_err(|_| hint.to_string())?;
    model["max_tokens"]
        .as_u64()
        .filter(|limit| *limit > 0)
        .ok_or_else(|| hint.to_string())
}
