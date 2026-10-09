use agent_runtime::config::{
    ClaudeConfig, CodexConfig, HarnessConfig, OpenCodeConfig, RuntimeConfig,
};
use std::{collections::BTreeMap, io::Write, path::PathBuf};

const KEYS: &[&str] = &[
    "ADMIN_AGENT_PROVIDER",
    "MODEL_PROVIDER",
    "MODEL_NAME",
    "MODEL_API_KEY",
    "MODEL_BASE_URL",
    "MODEL_API",
    "MODEL_SYSTEM_PROMPT",
    "HARNESS_MAX_TOKENS",
    "MODEL_THINKING",
    "MODEL_REASONING_EFFORT",
    "CONTEXT_MAX_TOKENS",
    "CODEX_BIN",
    "CLAUDE_BIN",
    "OPENCODE_BIN",
    "OPENCODE_MODEL",
    "OPENCODE_AGENT",
    "OPENCODE_AUTO_APPROVE",
    "OPENCODE_THINKING",
    "OPENCODE_STANDALONE",
    "AGENT_ENV_JSON",
];

#[derive(Default, Clone)]
pub(crate) struct Settings {
    values: BTreeMap<String, String>,
}
impl Settings {
    fn path() -> PathBuf {
        agent_runtime::paths::data_dir().join("default-agent.json")
    }
    pub(crate) fn load() -> Result<Self, String> {
        let mut settings = Self::load_saved(&agent_runtime::paths::data_dir())?;
        for key in KEYS {
            if let Ok(value) = std::env::var(key) {
                // Launcher dotenv values are defaults, not explicit operator overrides.
                // A saved interactive configuration must survive stale/blank template files.
                let defaults = std::env::var("CRABOT_CONFIG_DEFAULT_KEYS").unwrap_or_default();
                if defaults.split_whitespace().any(|item| item == *key)
                    && settings.values.contains_key(*key)
                {
                    continue;
                }
                settings.set(key, value);
            }
        }
        Ok(settings)
    }
    fn load_saved(dir: &std::path::Path) -> Result<Self, String> {
        let path = dir.join("default-agent.json");
        match std::fs::read(path) {
            Ok(bytes) => Ok(Self {
                values: serde_json::from_slice(&bytes).map_err(
                    |_| "Invalid default-agent.json; repair or move that file before configuring",
                )?,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub(crate) fn get(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or("")
    }
    pub(crate) fn public_view(&self) -> serde_json::Value {
        let mut values = self.values.clone();
        values.remove("MODEL_API_KEY");
        values.remove("CONTEXT_STRATEGY");
        if let Some(raw) = values.get("AGENT_ENV_JSON") {
            let masked = agent_runtime::environment::AgentEnvironment::from_json(raw)
                .map(|env| env.masked_json())
                .unwrap_or_else(|_| "{}".into());
            values.insert("AGENT_ENV_JSON".into(), masked);
        }
        serde_json::json!({"values": values, "has_api_key": !self.get("MODEL_API_KEY").is_empty()})
    }
    pub(crate) fn update(&mut self, mut values: BTreeMap<String, String>) -> Result<(), String> {
        // Accept old clients without retaining an obsolete user-facing strategy selector.
        values.remove("CONTEXT_STRATEGY");
        if values.keys().any(|key| !KEYS.contains(&key.as_str())) {
            return Err("unknown default Agent configuration field".into());
        }
        agent_runtime::context::ContextBudget::from_lookup(|key| {
            values
                .get(key)
                .cloned()
                .or_else(|| self.values.get(key).cloned())
        })?;
        if let Some(raw) = values.get("AGENT_ENV_JSON") {
            let env = agent_runtime::environment::AgentEnvironment::merge(
                self.get("AGENT_ENV_JSON"),
                raw,
            )?;
            values.insert("AGENT_ENV_JSON".into(), env.to_json());
        }
        self.values.extend(values);
        Ok(())
    }
    pub(crate) fn set(&mut self, key: &str, value: String) {
        self.values.insert(key.into(), value);
    }
    pub(crate) fn merge_values(
        previous: BTreeMap<String, String>,
        update: BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, String> {
        let mut settings = Self { values: previous };
        settings.update(update)?;
        Ok(settings.values)
    }
    pub(crate) fn runtime(&self) -> Result<RuntimeConfig, String> {
        let environment =
            agent_runtime::environment::AgentEnvironment::from_json(self.get("AGENT_ENV_JSON"))?;
        match self.get("ADMIN_AGENT_PROVIDER") {
            "" | "crabot" => Ok(RuntimeConfig::Crabot(HarnessConfig::from_lookup(|key| {
                if key == "CONTEXT_STRATEGY" {
                    return Some("intelligent".into());
                }
                environment.get(key).cloned().or_else(|| {
                    self.values
                        .get(key)
                        .cloned()
                        .or_else(|| std::env::var(key).ok())
                })
            })?)),
            "codex" => {
                let mut cfg = CodexConfig::from_env();
                cfg.environment = environment;
                if !self.get("CODEX_BIN").is_empty() {
                    cfg.binary = self.get("CODEX_BIN").into();
                }
                cfg.validate_launch()?;
                Ok(RuntimeConfig::Codex(cfg))
            }
            "claude" => {
                let mut cfg = ClaudeConfig::from_env();
                cfg.environment = environment;
                if !self.get("CLAUDE_BIN").is_empty() {
                    cfg.binary = self.get("CLAUDE_BIN").into();
                }
                cfg.validate_launch()?;
                Ok(RuntimeConfig::Claude(cfg))
            }
            "opencode" => {
                let cfg = self.opencode_with(environment, None)?;
                cfg.validate_launch()?;
                Ok(RuntimeConfig::OpenCode(cfg))
            }
            "mock" => Ok(RuntimeConfig::Mock),
            _ => Err("ADMIN_AGENT_PROVIDER must be crabot, codex, claude, opencode or mock".into()),
        }
    }
    /// Build the OpenCode runner independently of which runner is currently saved.
    ///
    /// The model picker is reachable while the operator is still choosing a runner,
    /// and a per-Agent override may name its own launcher, so neither the active
    /// provider nor the active configuration may gate this.
    pub(crate) fn opencode(&self, launcher: Option<&str>) -> Result<OpenCodeConfig, String> {
        let environment =
            agent_runtime::environment::AgentEnvironment::from_json(self.get("AGENT_ENV_JSON"))?;
        self.opencode_with(environment, launcher)
    }
    fn opencode_with(
        &self,
        environment: agent_runtime::environment::AgentEnvironment,
        launcher: Option<&str>,
    ) -> Result<OpenCodeConfig, String> {
        let mut cfg = OpenCodeConfig::from_env();
        cfg.environment = environment;
        // Saved settings win over the process environment, as for Codex and Claude.
        if !self.get("OPENCODE_BIN").is_empty() {
            cfg.binary = self.get("OPENCODE_BIN").into();
        }
        // A caller that owns its own launcher (a per-Agent override) outranks both.
        if let Some(launcher) = launcher.filter(|value| !value.trim().is_empty()) {
            cfg.binary = launcher.into();
        }
        if !self.get("OPENCODE_MODEL").is_empty() {
            cfg.model = self.get("OPENCODE_MODEL").into();
        }
        if !self.get("OPENCODE_AGENT").is_empty() {
            cfg.agent = self.get("OPENCODE_AGENT").into();
        }
        if let Some(raw) = self.values.get("OPENCODE_AUTO_APPROVE") {
            cfg.auto_approve = matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            );
        }
        // Only an explicit value moves these; an absent key keeps the runtime default,
        // so a configuration saved before they existed keeps behaving the same way.
        for (key, target) in [
            ("OPENCODE_THINKING", &mut cfg.thinking),
            ("OPENCODE_STANDALONE", &mut cfg.standalone),
        ] {
            if let Some(raw) = self.values.get(key) {
                *target = matches!(
                    raw.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
        }
        Ok(cfg)
    }
    pub(crate) fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }
    fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        self.write_to(path).map_err(|e| e.to_string())
    }
    fn write_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("configuration requires a parent directory"))?;
        std::fs::create_dir_all(parent)?;
        // tempfile creates an owner-only file on Unix; rename avoids partial configuration writes.
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, &self.values)?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_roundtrip_is_private_and_masked_edits_preserve_secrets() {
        let mut settings = Settings::default();
        settings
            .update(BTreeMap::from([
                ("ADMIN_AGENT_PROVIDER".into(), "claude".into()),
                (
                    "AGENT_ENV_JSON".into(),
                    r#"{"ANTHROPIC_API_KEY":"private-token","DROP":"old"}"#.into(),
                ),
            ]))
            .unwrap();
        assert!(!settings.public_view().to_string().contains("private-token"));
        settings
            .update(BTreeMap::from([(
                "AGENT_ENV_JSON".into(),
                r#"{"ANTHROPIC_API_KEY":null,"CUSTOM":"yes"}"#.into(),
            )]))
            .unwrap();
        let RuntimeConfig::Claude(config) = settings.runtime().unwrap() else {
            panic!("wrong provider")
        };
        assert_eq!(
            config.environment.get("ANTHROPIC_API_KEY").unwrap(),
            "private-token"
        );
        assert!(config.environment.get("DROP").is_none());
        let dir = tempfile::tempdir().unwrap();
        settings
            .save_to(&dir.path().join("default-agent.json"))
            .unwrap();
        assert_eq!(
            Settings::load_saved(dir.path())
                .unwrap()
                .get("AGENT_ENV_JSON"),
            settings.get("AGENT_ENV_JSON")
        );
    }
    #[test]
    fn loads_default_configuration_and_rejects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Settings::load_saved(dir.path()).unwrap().values.is_empty());
        let path = dir.path().join("default-agent.json");
        std::fs::write(&path, br#"{"ADMIN_AGENT_PROVIDER":"claude"}"#).unwrap();
        assert_eq!(
            Settings::load_saved(dir.path())
                .unwrap()
                .get("ADMIN_AGENT_PROVIDER"),
            "claude"
        );
        std::fs::write(&path, b"invalid").unwrap();
        assert!(
            Settings::load_saved(dir.path())
                .err()
                .unwrap()
                .contains("default-agent.json")
        );
    }
    #[test]
    fn validates_and_saves_private_literal_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = Settings::default();
        cfg.set("MODEL_PROVIDER", "invalid".into());
        assert!(cfg.runtime().is_err());
        cfg.set("MODEL_PROVIDER", "ollama".into());
        cfg.set("MODEL_NAME", "fixture".into());
        assert!(cfg.runtime().is_ok());
        cfg.set("MODEL_API_KEY", "literal $() secret".into());
        assert!(!cfg.public_view().to_string().contains("literal $() secret"));
        assert!(
            cfg.update(BTreeMap::from([("AGENT_WORKDIR".into(), "/".into())]))
                .is_err()
        );
        let path = dir.path().join("default-agent.json");
        cfg.save_to(&path).unwrap();
        let saved: BTreeMap<String, String> =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["MODEL_API_KEY"], "literal $() secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        for provider in ["codex", "claude", "opencode", "mock"] {
            cfg.set("ADMIN_AGENT_PROVIDER", provider.into());
            assert!(cfg.runtime().is_ok());
        }
    }
}
