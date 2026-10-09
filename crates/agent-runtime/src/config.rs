//! Public configuration facade. Each provider owns its concrete configuration.
use crate::RuntimeKind;
pub use crate::providers::executable::LauncherAvailability;
pub use crate::providers::opencode::models::{
    Catalog as OpenCodeCatalog, Model as OpenCodeModel, Source as OpenCodeCatalogSource,
    discover as opencode_models,
};
pub use crate::providers::{
    claude::config::ClaudeConfig,
    codex::config::CodexConfig,
    crabot::config::{HarnessConfig, ModelApi},
    opencode::config::OpenCodeConfig,
};

pub fn default_crabot_system_prompt() -> &'static str {
    crate::providers::crabot::prompt::default_system_prompt()
}
use std::path::PathBuf;

pub(crate) fn workdir() -> PathBuf {
    crate::paths::workdir()
}

#[derive(Clone)]
pub enum RuntimeConfig {
    Mock,
    Crabot(HarnessConfig),
    Claude(ClaudeConfig),
    Codex(CodexConfig),
    OpenCode(OpenCodeConfig),
}
impl RuntimeConfig {
    pub(crate) fn environment(&self) -> crate::environment::AgentEnvironment {
        match self {
            Self::Mock => Default::default(),
            Self::Crabot(cfg) => cfg.environment.clone(),
            Self::Codex(cfg) => cfg.environment.clone(),
            Self::Claude(cfg) => cfg.environment.clone(),
            Self::OpenCode(cfg) => cfg.environment.clone(),
        }
    }
    /// Apply the host's execution-local directory uniformly for all providers.
    pub fn with_workspace(mut self) -> Self {
        if let Some(root) = crate::workspace::WorkspaceSettings::root() {
            match &mut self {
                Self::Mock => (),
                Self::Crabot(config) => config.root = root,
                Self::Codex(config) => config.workdir = root,
                Self::Claude(config) => config.workdir = root,
                Self::OpenCode(config) => config.workdir = root,
            }
        }
        self
    }
    pub fn kind(&self) -> RuntimeKind {
        match self {
            Self::Mock => RuntimeKind::Mock,
            Self::Crabot(_) => RuntimeKind::Crabot,
            Self::Codex(_) => RuntimeKind::Codex,
            Self::Claude(_) => RuntimeKind::Claude,
            Self::OpenCode(_) => RuntimeKind::OpenCode,
        }
    }
    pub fn from_env(kind: RuntimeKind) -> Result<Self, String> {
        let environment = crate::environment::AgentEnvironment::from_json(
            &std::env::var("AGENT_ENV_JSON").unwrap_or_default(),
        )?;
        Ok(match kind {
            RuntimeKind::Mock => Self::Mock,
            RuntimeKind::Crabot => Self::Crabot(HarnessConfig::from_env()?),
            RuntimeKind::Claude => {
                let mut cfg = ClaudeConfig::from_env();
                cfg.environment = environment;
                Self::Claude(cfg)
            }
            RuntimeKind::Codex => {
                let mut cfg = CodexConfig::from_env();
                cfg.environment = environment;
                Self::Codex(cfg)
            }
            RuntimeKind::OpenCode => {
                let mut cfg = OpenCodeConfig::from_env();
                cfg.environment = environment;
                Self::OpenCode(cfg)
            }
        })
    }
}
