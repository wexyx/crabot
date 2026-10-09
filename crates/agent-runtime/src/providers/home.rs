//! Locating a provider CLI's own configuration directory.

/// The invoking user's home directory.
///
/// Provider CLIs are launched with a scrubbed `HOME` so they cannot wander the rest of
/// the home directory. Their configuration is reached instead through the vendor's own
/// directory variable, which is why this reads the real `HOME` and not the child's.
pub(crate) fn home_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// A CLI's configuration directory, or `None` when it does not exist.
///
/// Returning `None` rather than a path is deliberate: a directory that does not exist
/// would make the CLI start without its skills, which looks like a Crabot bug rather
/// than a CLI that was never installed. Callers fall back to a scratch home instead.
pub(crate) fn config_dir(vendor: &str) -> Option<std::path::PathBuf> {
    let dir = home_dir().join(vendor);
    dir.is_dir().then_some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_missing_config_directory_is_absent_rather_than_a_broken_path() {
        assert_eq!(config_dir(".crabot-does-not-exist"), None);
        // HOME itself may be unset in a sandbox; that must not panic.
        assert!(home_dir().is_absolute() || home_dir().as_os_str().is_empty());
    }
}
