use std::path::{Path, PathBuf};

/// Shared location for generated files. Never follows directory symlinks.
pub(crate) fn temporary_dir(root: &Path) -> Result<PathBuf, String> {
    let instance = std::env::var("CRABOT_INSTANCE").ok();
    create(root, instance.as_deref())
}

fn create(root: &Path, instance: Option<&str>) -> Result<PathBuf, String> {
    let name = match instance.filter(|value| !value.is_empty()) {
        Some(value)
            if value.len() <= 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') =>
        {
            format!(".crabot_{value}")
        }
        Some(_) => return Err("invalid Crabot instance name for temporary directory".into()),
        None => ".crabot".into(),
    };
    let mut path = root.canonicalize().map_err(|e| e.to_string())?;
    for part in [&name, "tmp"] {
        path.push(part);
        match std::fs::create_dir(&path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("Crabot temporary directory must be a directory, not a symlink".into());
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_and_named_instances_have_separate_workdir_temp_directories() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        assert_eq!(create(&root, None).unwrap(), root.join(".crabot/tmp"));
        assert_eq!(
            create(&root, Some("demo")).unwrap(),
            root.join(".crabot_demo/tmp")
        );
        assert!(create(&root, Some("../outside")).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_redirecting_temporary_files_outside_workspace() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join(".crabot")).unwrap();
        assert!(create(root.path(), None).is_err());
        assert!(!outside.path().join("tmp").exists());
    }
}
