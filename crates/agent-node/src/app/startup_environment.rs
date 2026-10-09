use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const KEYS: &[&str] = &[
    "CRABOT_DATA_DIR",
    "CRABOT_HISTORY_BUFFER_MIB",
    "BIND_ADDR",
    "AGENT_MODE",
    "AGENT_NAME",
    "AGENT_PROVIDER",
    "ADMIN_AGENT_PROVIDER",
    "AGENT_WORKDIR",
    "AGENT_OUTSIDE_ACCESS",
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
    "WEB_CONFIG_ORIGINS",
    "WEB_CONFIG_DIR",
    "CRABOT_EXECUTION_PROFILES_JSON",
    "CRABOT_EXECUTION_PROFILE",
    "CRABOT_EXECUTION_ALLOWED_PROFILES",
    "CRABOT_SANDBOX_PROFILES_JSON",
    "CRABOT_SANDBOX_PROFILE",
    "CRABOT_SANDBOX_ALLOWED_PROFILES",
    "CRABOT_SYSTEM_SKILLS_DIR",
    "NODE_LINKS_JSON",
    "CODEX_BIN",
    "CLAUDE_BIN",
    "AGENT_ENV_JSON",
];

/// Configuration discovery is performed once, before any runtime threads exist.
pub(super) struct StartupEnvironment;
impl StartupEnvironment {
    pub(super) fn launch_dir() -> Result<PathBuf, String> {
        std::env::var_os("CRABOT_LAUNCH_DIR")
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| e.to_string())
    }
    pub(super) fn load(explicit_data: Option<PathBuf>) -> Result<(), String> {
        let cwd = Self::launch_dir()?;
        let local = read(&cwd.join(".agent.env"))?;
        let data_dir = absolute(
            &cwd,
            explicit_data
                .or_else(|| {
                    std::env::var_os("CRABOT_DATA_DIR")
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                })
                .or_else(|| {
                    local
                        .get("CRABOT_DATA_DIR")
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                })
                .unwrap_or_else(|| agent_runtime::paths::user_home().join(".crabot")),
        );
        let mut values = read(&data_dir.join(".agent.env"))?;
        // The selected instance cannot redirect its own configuration lookup.
        values.remove("CRABOT_DATA_DIR");
        values.extend(local);
        let mut defaults = std::env::var("CRABOT_CONFIG_DEFAULT_KEYS").unwrap_or_default();
        for (key, value) in values {
            if key == "CRABOT_DATA_DIR" || std::env::var_os(&key).is_some() {
                continue;
            }
            defaults.push(' ');
            defaults.push_str(&key);
            // Called exclusively by startup::configure before Tokio is constructed.
            unsafe {
                std::env::set_var(key, value);
            }
        }
        for (key, bundled) in [
            ("WEB_CONFIG_DIR", "CRABOT_BUNDLED_WEB_DIR"),
            (
                "CRABOT_SYSTEM_SKILLS_DIR",
                "CRABOT_BUNDLED_SYSTEM_SKILLS_DIR",
            ),
        ] {
            if std::env::var_os(key).is_none() {
                if let Some(value) = std::env::var_os(bundled) {
                    unsafe {
                        std::env::set_var(key, value);
                    }
                }
            }
        }
        unsafe {
            std::env::set_var("CRABOT_DATA_DIR", data_dir);
            std::env::set_var("CRABOT_CONFIG_DEFAULT_KEYS", defaults);
        }
        Ok(())
    }
}

pub(super) fn absolute(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn read(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let mut values = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "{}:{}: expected KEY=value",
                path.display(),
                index + 1
            ));
        };
        let key = key.trim();
        if !KEYS.contains(&key) {
            continue;
        }
        let value = value.trim();
        if value.contains('\0') {
            return Err(format!("{}:{}: invalid value", path.display(), index + 1));
        }
        let value = if !value.is_empty()
            && matches!(
                key,
                "CRABOT_DATA_DIR" | "AGENT_WORKDIR" | "WEB_CONFIG_DIR" | "CRABOT_SYSTEM_SKILLS_DIR"
            ) {
            absolute(path.parent().unwrap(), value.into())
                .to_string_lossy()
                .into_owned()
        } else {
            value.to_owned()
        };
        values.insert(key.to_owned(), value);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_are_literal_and_paths_are_relative_to_their_config_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".agent.env");
        std::fs::write(
            &path,
            "# comment\nMODEL_API_KEY=literal $(touch nope)\nAGENT_WORKDIR=work\nCRABOT_HISTORY_BUFFER_MIB=2048\nHOME=ignored\n",
        )
        .unwrap();
        let values = read(&path).unwrap();
        assert_eq!(values["MODEL_API_KEY"], "literal $(touch nope)");
        assert_eq!(values["CRABOT_HISTORY_BUFFER_MIB"], "2048");
        assert_eq!(
            values["AGENT_WORKDIR"],
            dir.path().join("work").to_string_lossy()
        );
        assert!(!values.contains_key("HOME"));
        assert!(!dir.path().join("nope").exists());
    }
}
