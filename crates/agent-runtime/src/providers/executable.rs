use super::launch_command::LaunchCommand;
use std::path::{Path, PathBuf};

/// Read-only launcher discovery; never executes a shell, vendor CLI or installer.
pub struct LauncherAvailability;
impl LauncherAvailability {
    pub fn check(command: &str) -> Result<PathBuf, String> {
        Self::resolve(
            command,
            std::env::var_os("PATH").as_deref(),
            &crate::paths::workdir(),
        )
    }
    fn resolve(
        command: &str,
        search: Option<&std::ffi::OsStr>,
        root: &Path,
    ) -> Result<PathBuf, String> {
        let launch = LaunchCommand::parse(Path::new(command))?;
        let binary = launch.binary();
        let candidates: Vec<_> = if binary.is_absolute() || binary.components().count() > 1 {
            vec![root.join(binary)]
        } else {
            search
                .map(|paths| {
                    std::env::split_paths(paths)
                        .map(|dir| root.join(dir).join(binary))
                        .collect()
                })
                .unwrap_or_default()
        };
        candidates
            .into_iter()
            .find(|path| Self::executable(path))
            .ok_or_else(|| {
                format!(
                    "未找到可执行程序 {}（未安装、不在 PATH 中或没有执行权限）",
                    binary.display()
                )
            })
    }
    fn executable(path: &Path) -> bool {
        let Ok(metadata) = path.metadata() else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn detects_paths_arguments_and_permissions_without_running_commands() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("custom cli");
        std::fs::write(&file, "#!/bin/sh\nexit 99\n").unwrap();
        let command = format!("\"{}\" --model test", file.display());
        assert!(LauncherAvailability::resolve(&command, None, dir.path()).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            LauncherAvailability::resolve(&command, None, dir.path()).unwrap(),
            file
        );
        assert!(
            LauncherAvailability::resolve("missing-cli", Some(dir.path().as_os_str()), dir.path())
                .is_err()
        );
        assert!(
            LauncherAvailability::resolve(
                "\"custom cli\"",
                Some(dir.path().as_os_str()),
                dir.path()
            )
            .is_ok()
        );
        assert!(
            LauncherAvailability::resolve(
                "custom && touch marker",
                Some(dir.path().as_os_str()),
                dir.path()
            )
            .is_err()
        );
        assert!(!dir.path().join("marker").exists());
    }
}
