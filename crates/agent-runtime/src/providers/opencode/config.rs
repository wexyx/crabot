use crate::config::workdir;
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct OpenCodeConfig {
    pub binary: PathBuf,
    pub environment: crate::environment::AgentEnvironment,
    pub model: String,
    pub agent: String,
    /// Mirrors Claude's `plan`/`bypassPermissions` split: OpenCode auto-approves only
    /// what it has not been explicitly denied.
    pub auto_approve: bool,
    /// Deliberation before the first token. It costs time on every turn, so a human
    /// chooses it rather than the runtime assuming a reasoning model.
    pub thinking: bool,
    /// Run against a private OpenCode server instead of the shared background service.
    ///
    /// The shared service is cheaper by roughly a second per turn when a client can
    /// reach it, but Crabot launches OpenCode with a scrubbed HOME, and the service
    /// registration lives in that HOME. Without it the client cannot find the service
    /// and waits on starting one instead of failing fast, so the private server is the
    /// default here. Clear `OPENCODE_STANDALONE` only where the registration is
    /// reachable from the launched environment.
    pub standalone: bool,
    pub workdir: PathBuf,
}
/// Read a tri-state flag: absent keeps the caller's default.
fn flag(name: &str) -> Option<bool> {
    env::var(name).ok().map(|v| parse_flag(&v))
}
/// Every spelling an operator might use has to read the same way.
fn parse_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
impl OpenCodeConfig {
    pub fn validate_launch(&self) -> Result<(), String> {
        self.launch().map(|_| ())
    }
    pub(crate) fn launch(&self) -> Result<super::super::launch_command::LaunchCommand, String> {
        let command = super::super::launch_command::LaunchCommand::parse(&self.binary)?;
        // Crabot owns task framing, session continuity, output format and approval.
        command.reject(&[
            "run",
            "--format",
            "--prompt",
            "--thinking",
            "--auto",
            "--continue",
            "-c",
            "--session",
            "-s",
            "--fork",
            "--command",
            "--share",
            "--attach",
            "--dir",
            "--port",
            "--password",
            "--username",
            "--print-logs",
            "--log-level",
            "--pure",
        ])?;
        Ok(command)
    }
    pub fn from_env() -> Self {
        Self {
            environment: Default::default(),
            binary: env::var("OPENCODE_BIN")
                .unwrap_or_else(|_| "opencode".into())
                .into(),
            model: env::var("OPENCODE_MODEL").unwrap_or_default(),
            agent: env::var("OPENCODE_AGENT").unwrap_or_default(),
            // Default to the restrictive side: without an explicit human decision an
            // OpenCode worker must not be able to auto-approve its own tool calls.
            auto_approve: flag("OPENCODE_AUTO_APPROVE").unwrap_or(false),
            // Deliberation used to be forced on for every model. It stays on by default
            // so no turn silently changes behaviour, but it is now a decision, not an
            // assumption: models without reasoning gain nothing from it.
            thinking: flag("OPENCODE_THINKING").unwrap_or(true),
            // The shared service cannot be discovered from a scrubbed HOME, and waiting
            // on a server that never registers is worse than booting a private one.
            standalone: flag("OPENCODE_STANDALONE").unwrap_or(true),
            workdir: workdir(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// These two flags moved the measured latency of every turn, so the side they fall
    /// on when nothing is configured is pinned rather than assumed.
    #[test]
    fn deliberation_stays_on_and_a_private_server_is_booted_by_default() {
        let config = OpenCodeConfig::from_env();
        assert!(config.thinking, "deliberation must not change silently");
        assert!(
            config.standalone,
            "a scrubbed HOME cannot find the shared service registration"
        );
        assert!(
            !config.auto_approve,
            "a worker must not approve its own tools by default"
        );
    }
    /// The parser is tested directly rather than through the process environment, which
    /// is unsafe to mutate from a parallel test and would make the result a race.
    #[test]
    fn flags_accept_the_usual_spellings_and_read_a_blank_as_off() {
        for value in ["1", "true", "TRUE", "yes", "on", " on "] {
            assert!(parse_flag(value), "{value:?}");
        }
        for value in ["0", "false", "no", "off", "", "  ", "maybe"] {
            assert!(!parse_flag(value), "{value:?}");
        }
    }
}
