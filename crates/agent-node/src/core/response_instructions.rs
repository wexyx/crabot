use serde_json::Value;
pub(super) const DEFAULT: &str = include_str!("../../../../conf/response.md");
pub(super) fn current(configured: Option<&str>) -> Result<String, String> {
    match configured {
        Some(text) if text.trim() != DEFAULT.trim() => Ok(text.to_owned()),
        _ => agent_runtime::prompts::PromptStore::instance().read("response"),
    }
}
pub(super) fn resolve(input: &Value, previous: Option<&Value>) -> Result<String, String> {
    match input.get("response_instructions") {
        Some(value) => {
            let text = value.as_str().ok_or("response_instructions must be text")?;
            if text.len() > 8192 {
                return Err("回复要求不能超过 8192 字节".into());
            }
            Ok(text.to_owned())
        }
        None => Ok(previous
            .and_then(|v| v["response_instructions"].as_str())
            .unwrap_or(DEFAULT)
            .into()),
    }
}
