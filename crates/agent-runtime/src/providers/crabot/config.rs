use std::{env, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelApi {
    Chat,
    Responses,
    Anthropic,
}

#[derive(Clone)]
pub struct HarnessConfig {
    pub environment: crate::environment::AgentEnvironment,
    pub context: crate::context::ContextBudget,
    pub system_prompt: String,
    pub api: ModelApi,
    pub base: String,
    pub key: String,
    pub model: String,
    pub max_tokens: Option<u64>,
    pub deepseek_effort: Option<String>,
    pub root: PathBuf,
}

impl HarnessConfig {
    /// Automatic output headroom is only used for local context accounting;
    /// it is not an output limit sent to the model.
    pub(super) fn input_limit(&self) -> Result<usize, String> {
        let reserve = self
            .max_tokens
            .unwrap_or((self.context.input_limit(0)? / 4) as u64);
        self.context.input_limit(reserve)
    }

    pub(crate) fn validate(mut self) -> Result<Self, String> {
        if !(self.base.starts_with("http://") || self.base.starts_with("https://")) {
            return Err("MODEL_BASE_URL must be an HTTP(S) URL".into());
        }
        if self.model.is_empty() {
            return Err("MODEL_NAME is required; select a model with tool-calling support".into());
        }
        if self.system_prompt.trim().is_empty() {
            self.system_prompt = super::prompt::default_system_prompt().into();
        }
        if self.max_tokens == Some(0) {
            return Err("HARNESS_MAX_TOKENS must be positive".into());
        }
        self.input_limit()?;
        self.root = std::fs::canonicalize(self.root).map_err(|_| "AGENT_WORKDIR does not exist")?;
        self.base = self.base.trim_end_matches('/').into();
        Ok(self)
    }
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let environment = crate::environment::AgentEnvironment::from_json(
            &get("AGENT_ENV_JSON").unwrap_or_default(),
        )?;
        let original = get;
        let get = |key: &str| environment.get(key).cloned().or_else(|| original(key));
        let vendor = get("MODEL_PROVIDER").unwrap_or_else(|| "openai".into());
        let (api, base) = match vendor.as_str() {
            "openai" => (ModelApi::Responses, "https://api.openai.com/v1"),
            "anthropic" => (ModelApi::Anthropic, "https://api.anthropic.com/v1"),
            "gemini" => (
                ModelApi::Chat,
                "https://generativelanguage.googleapis.com/v1beta/openai",
            ),
            "deepseek" => (ModelApi::Chat, "https://api.deepseek.com/v1"),
            "qwen" => (
                ModelApi::Chat,
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ),
            "ark" => (ModelApi::Chat, "https://ark.cn-beijing.volces.com/api/v3"),
            "ollama" => (ModelApi::Chat, "http://localhost:11434/v1"),
            "compatible" => (ModelApi::Chat, ""),
            _ => return Err("unknown MODEL_PROVIDER".into()),
        };
        let api = match get("MODEL_API").filter(|v| !v.is_empty()).as_deref() {
            None => api,
            Some("chat") => ModelApi::Chat,
            Some("responses") => ModelApi::Responses,
            Some("anthropic") => ModelApi::Anthropic,
            Some(_) => return Err("MODEL_API must be chat, responses, or anthropic".into()),
        };
        let base = get("MODEL_BASE_URL")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| base.into());
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err("MODEL_BASE_URL must be an HTTP(S) URL".into());
        }
        let model = get("MODEL_NAME")
            .filter(|m| !m.is_empty())
            .ok_or("MODEL_NAME is required; select a model with tool-calling support")?;
        let key = get("MODEL_API_KEY").unwrap_or_default();
        if key.is_empty() && vendor != "ollama" {
            return Err("MODEL_API_KEY is required".into());
        }
        let context = crate::context::ContextBudget::from_lookup(|key| {
            if key == "CONTEXT_STRATEGY" {
                Some("intelligent".into())
            } else {
                get(key)
            }
        })?;
        let system_prompt = get("MODEL_SYSTEM_PROMPT")
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| super::prompt::default_system_prompt().into());
        let max_tokens = get("HARNESS_MAX_TOKENS")
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.trim().parse::<u64>())
            .transpose()
            .map_err(|_| "invalid HARNESS_MAX_TOKENS")?;
        if max_tokens == Some(0) {
            return Err("HARNESS_MAX_TOKENS must be positive".into());
        }
        let deepseek_effort = if vendor == "deepseek" && api != ModelApi::Anthropic {
            let mode = get("MODEL_THINKING")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "disabled".into());
            let effort = get("MODEL_REASONING_EFFORT")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "low".into());
            if !["low", "high", "max"].contains(&effort.as_str()) {
                return Err("MODEL_REASONING_EFFORT must be low, high or max".into());
            }
            Some(match mode.as_str() {
                "disabled" => "none".into(),
                "enabled" => effort,
                _ => return Err("MODEL_THINKING must be enabled or disabled".into()),
            })
        } else {
            None
        };
        let root = std::fs::canonicalize(
            get("AGENT_WORKDIR")
                .map(PathBuf::from)
                .unwrap_or_else(crate::paths::user_home),
        )
        .map_err(|_| "AGENT_WORKDIR does not exist")?;
        Self {
            environment,
            context,
            system_prompt,
            api,
            base: base.trim_end_matches('/').into(),
            key,
            model,
            max_tokens,
            deepseek_effort,
            root,
        }
        .validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(vendor: &str, extra: &[(&str, &str)]) -> Result<HarnessConfig, String> {
        HarnessConfig::from_lookup(|key| {
            if let Some((_, v)) = extra.iter().find(|(k, _)| *k == key) {
                return Some((*v).into());
            }
            match key {
                "MODEL_PROVIDER" => Some(vendor.into()),
                "MODEL_NAME" => Some("fixture".into()),
                "MODEL_API_KEY" => Some("fixture-key".into()),
                "AGENT_WORKDIR" => Some(env!("CARGO_MANIFEST_DIR").into()),
                _ => None,
            }
        })
    }
    #[test]
    fn vendor_defaults_and_protocol_overrides_remain_compatible() {
        for (vendor, api) in [
            ("openai", ModelApi::Responses),
            ("anthropic", ModelApi::Anthropic),
            ("gemini", ModelApi::Chat),
            ("deepseek", ModelApi::Chat),
            ("qwen", ModelApi::Chat),
            ("ark", ModelApi::Chat),
            ("ollama", ModelApi::Chat),
        ] {
            let cfg = config(vendor, &[]).unwrap();
            assert_eq!(cfg.api, api);
            assert!(cfg.base.starts_with("http"));
        }
        let cfg = config(
            "compatible",
            &[
                ("MODEL_BASE_URL", "http://localhost:1234/v1/"),
                ("MODEL_API", "responses"),
            ],
        )
        .unwrap();
        assert_eq!(cfg.api, ModelApi::Responses);
        assert_eq!(cfg.base, "http://localhost:1234/v1");
    }

    #[test]
    fn system_prompt_is_configurable_with_a_default() {
        assert_eq!(
            config("openai", &[]).unwrap().system_prompt,
            crate::config::default_crabot_system_prompt()
        );
        let cfg = config("openai", &[("MODEL_SYSTEM_PROMPT", "custom instructions")]).unwrap();
        assert_eq!(cfg.system_prompt, "custom instructions");
    }

    #[test]
    fn protocol_requests_use_the_configured_system_prompt() {
        use crate::providers::crabot::protocol::ProtocolFactory;
        let http = reqwest::Client::new();
        for api in [ModelApi::Chat, ModelApi::Responses, ModelApi::Anthropic] {
            let mut cfg = config("openai", &[("MODEL_SYSTEM_PROMPT", "custom system")]).unwrap();
            cfg.api = api;
            let request = ProtocolFactory::create(api)
                .request(&http, &cfg, &[], &crate::tools::ToolRegistry::new())
                .build()
                .unwrap();
            let body: serde_json::Value =
                serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
            match api {
                ModelApi::Chat => assert_eq!(body["messages"][0]["content"], "custom system"),
                ModelApi::Responses => assert_eq!(body["instructions"], "custom system"),
                ModelApi::Anthropic => assert_eq!(body["system"], "custom system"),
            }
        }
    }

    #[test]
    fn deepseek_speed_defaults_and_independent_output_limit() {
        use crate::providers::crabot::protocol::ProtocolFactory;
        let http = reqwest::Client::new();
        for api in ["chat", "responses"] {
            for (mode, effort) in [("disabled", "none"), ("enabled", "low")] {
                let cfg = config(
                    "deepseek",
                    &[
                        ("MODEL_API", api),
                        ("MODEL_THINKING", mode),
                        ("HARNESS_MAX_TOKENS", "2048"),
                    ],
                )
                .unwrap();
                let request = ProtocolFactory::create(cfg.api)
                    .request(&http, &cfg, &[], &crate::tools::ToolRegistry::new())
                    .build()
                    .unwrap();
                let body: serde_json::Value =
                    serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
                if api == "chat" {
                    assert_eq!(body["max_tokens"], 2048);
                    assert_eq!(body["thinking"]["type"], mode);
                    assert_eq!(body["reasoning_effort"], effort);
                    assert!(body.get("max_output_tokens").is_none());
                } else {
                    assert_eq!(body["max_output_tokens"], 2048);
                    assert_eq!(body["reasoning"]["effort"], effort);
                }
            }
        }
        let cfg = config("deepseek", &[]).unwrap();
        assert_eq!(cfg.deepseek_effort.as_deref(), Some("none"));
        assert_eq!(cfg.max_tokens, None);
        assert!(config("openai", &[]).unwrap().deepseek_effort.is_none());
        assert!(config("deepseek", &[("HARNESS_MAX_TOKENS", "0")]).is_err());
        assert!(config("deepseek", &[("MODEL_THINKING", "invalid")]).is_err());
        assert!(config("deepseek", &[("MODEL_REASONING_EFFORT", "invalid")]).is_err());
    }
    #[test]
    fn config_validation_does_not_mutate_process_environment() {
        assert!(config("compatible", &[]).is_err());
        assert!(config("unknown", &[]).is_err());
        assert!(config("openai", &[("MODEL_API", "invalid")]).is_err());
        assert!(config("openai", &[("MODEL_API_KEY", "")]).is_err());
        assert!(config("ollama", &[("MODEL_API_KEY", "")]).is_ok());
    }
}
