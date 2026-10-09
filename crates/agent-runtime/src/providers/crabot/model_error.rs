pub(super) fn model_error(text: &str) -> String {
    if crate::is_token_insufficient(text) || text.contains("max_output_tokens") {
        format!("TOKEN_INSUFFICIENT: {text}")
    } else {
        format!("model request failed: {text}")
    }
}
