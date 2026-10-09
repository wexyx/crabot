use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::Command;

/// Host process launcher. Temporary environment and process cleanup are not filesystem isolation.
pub(crate) struct NativeCommand {
    scratch: tempfile::TempDir,
}
impl NativeCommand {
    pub(crate) async fn import_credential(
        &self,
        root: &Path,
        source: &Path,
        destination: &Path,
    ) -> Result<(), String> {
        if !source.is_file() {
            return Ok(());
        }
        let workspace = crate::workspace::Workspace::new(
            root.into(),
            crate::workspace::OutsideAccess::from_env()?,
        )?;
        let approved = workspace
            .authorize(
                source,
                "provider credential read (copy into temporary runtime)",
            )
            .await?;
        let target = self.scratch.path().join(destination);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(approved, target).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(crate) fn new(root: &Path) -> Result<Self, String> {
        Ok(Self {
            scratch: tempfile::Builder::new()
                .prefix("crabot-process-")
                .tempdir_in(crate::workspace::temporary_dir(root)?)
                .map_err(|e| e.to_string())?,
        })
    }
    pub(crate) fn scratch(&self) -> &Path {
        self.scratch.path()
    }
    pub(crate) fn command(&self, binary: &Path, root: &Path) -> Result<Command, String> {
        self.build_command(binary, root, false)
    }
    pub(crate) fn shell_command(
        &self,
        root: &Path,
        home: &Path,
        interactive: bool,
    ) -> Result<Command, String> {
        std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let mut command = self.build_command(Path::new("/bin/sh"), root, interactive)?;
        command
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("TERM", "dumb");
        crate::environment::AgentEnvironment::apply(&mut command);
        Ok(command)
    }
    fn build_command(&self, binary: &Path, root: &Path, terminal: bool) -> Result<Command, String> {
        let binary = resolve_binary(binary)?;
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("workdir is not a directory".into());
        }
        let scratch = self
            .scratch
            .path()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let mut command = Command::new(&binary);
        command
            .current_dir(&root)
            .env_clear()
            .env("HOME", &scratch)
            .env("TMPDIR", &scratch)
            .env("CRABOT_TMP_DIR", crate::workspace::temporary_dir(&root)?)
            .env("CRABOT_DATA_DIR", crate::paths::data_dir())
            .env("MAC_CHROMIUM_TMPDIR", &scratch)
            .env("XDG_CONFIG_HOME", scratch.join("config"))
            .env("XDG_CACHE_HOME", scratch.join("cache"))
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env(
                "LANG",
                std::env::var_os("LANG").unwrap_or_else(|| "en_US.UTF-8".into()),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        {
            for key in [
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "ALL_PROXY",
                "NO_PROXY",
                "http_proxy",
                "https_proxy",
                "all_proxy",
                "no_proxy",
            ] {
                if let Some(value) = std::env::var_os(key) {
                    command.env(key, value);
                }
            }
        }
        #[cfg(unix)]
        if !terminal {
            command.process_group(0);
        }
        crate::environment::AgentEnvironment::apply(&mut command);
        Ok(command)
    }
}
fn resolve_binary(binary: &Path) -> Result<PathBuf, String> {
    let resolved = if binary.components().count() > 1 {
        binary.canonicalize().ok()
    } else {
        std::env::split_paths(
            &crate::environment::AgentEnvironment::lookup("PATH").unwrap_or_default(),
        )
        .find_map(|p| p.join(binary).canonicalize().ok())
    };
    resolved
        .filter(|p| p.is_file())
        .ok_or_else(|| format!("executable not found: {}", binary.display()))
}
