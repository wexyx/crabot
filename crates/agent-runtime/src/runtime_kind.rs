//! Supported provider identities and configuration-name compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeKind {
    Mock,
    Crabot,
    Claude,
    Codex,
    OpenCode,
}
impl RuntimeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Crabot => "crabot",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }
}
impl std::str::FromStr for RuntimeKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "mock" => Ok(Self::Mock),
            "crabot" | "builtin" => Ok(Self::Crabot),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "opencode" => Ok(Self::OpenCode),
            other => Err(format!(
                "Unknown AGENT_PROVIDER '{other}'; choose crabot, claude, codex, opencode, or mock"
            )),
        }
    }
}
