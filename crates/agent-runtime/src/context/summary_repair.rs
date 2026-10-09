use serde_json::{Value, json};

const KEYS: [&str; 7] = [
    "goals",
    "constraints",
    "decisions",
    "completed",
    "pending",
    "risks",
    "references",
];
pub(super) fn parse(text: &str) -> Result<Value, String> {
    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .and_then(|s| s.trim().strip_suffix("```"))
        .unwrap_or(text)
        .trim();
    let value: Value = serde_json::from_str(text)
        .map_err(|_| "智能压缩返回的摘要不是有效 JSON，原始上下文未修改")?;
    let object = value.as_object().ok_or("摘要必须是 JSON 对象")?;
    if object.len() != KEYS.len()
        || KEYS.iter().any(|key| {
            !object
                .get(*key)
                .and_then(Value::as_array)
                .is_some_and(|items| items.iter().all(Value::is_string))
        })
    {
        return Err("智能压缩摘要结构无效，原始上下文未修改".into());
    }
    Ok(value)
}

/// Last-resort reduction of a valid generated digest, never of original messages.
/// Keep all categories and an explicit loss notice. Serialized bytes are the budget.
pub(super) fn fit(text: &str, limit: usize) -> Result<String, String> {
    let original = parse(text)?;
    if original.to_string().len() <= limit {
        return Ok(original.to_string());
    }
    let mut result = json!({"goals":[],"constraints":[],"decisions":[],"completed":[],"pending":[],"risks":["Digest shortened after retries. Details omitted: use find(target=history) before acting on uncertain constraints or repeating tool effects."],"references":[]});
    let available = limit
        .checked_sub(result.to_string().len() + 32)
        .ok_or("摘要空间不足，原始上下文未修改")?;
    let quota = available / KEYS.len();
    for key in KEYS {
        let target = result[key].as_array_mut().unwrap();
        let mut used = 0;
        for item in original[key].as_array().unwrap() {
            let remaining = quota.saturating_sub(used);
            if remaining < 32 {
                break;
            }
            let text = item.as_str().unwrap();
            let mut candidate = text.to_owned();
            if json!(candidate).to_string().len() + 1 > remaining {
                let mut bytes = text.len();
                loop {
                    bytes = bytes.saturating_mul(3) / 4;
                    candidate = format!("{}… [omitted]", super::contract::prefix(text, bytes));
                    if json!(candidate).to_string().len() + 1 <= remaining || bytes == 0 {
                        break;
                    }
                }
            }
            let size = json!(candidate).to_string().len() + 1;
            if size > remaining {
                break;
            }
            used += size;
            target.push(json!(candidate));
        }
    }
    let text = result.to_string();
    if text.len() > limit {
        return Err("摘要空间不足，原始上下文未修改".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_shape_unicode_categories_and_measured_budget() {
        let mut summary = json!({});
        for key in KEYS {
            summary[key] = json!(["中文\"\\\n".repeat(500)]);
        }
        for size in [512, 682, 2048, 8192] {
            let text = fit(&summary.to_string(), size).unwrap();
            assert!(text.len() <= size);
            let parsed = parse(&text).unwrap();
            assert!(parsed["risks"][0].as_str().unwrap().contains("omitted"));
            for key in KEYS {
                assert!(!parsed[key].as_array().unwrap().is_empty());
            }
        }
        assert!(fit("invalid", 1024).is_err());
        assert!(fit("{}", 1024).is_err());
        assert!(fit(&summary.to_string(), 50).is_err());
    }
    #[test]
    fn valid_short_digest_is_unchanged() {
        let text=json!({"goals":["keep"],"constraints":[],"decisions":[],"completed":[],"pending":[],"risks":[],"references":[]}).to_string();
        assert_eq!(fit(&text, 1024).unwrap(), text);
    }
}
