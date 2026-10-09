use super::{PromptDefinition, definitions};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

static WRITES: Mutex<()> = Mutex::new(());
const MAX_BYTES: u64 = 128 * 1024;

/// Instance-local text files. Read per task; never cache another worker's edits.
pub struct PromptStore {
    directory: PathBuf,
}

impl PromptStore {
    pub fn new(instance: impl AsRef<Path>) -> Self {
        Self {
            directory: instance.as_ref().join("conf"),
        }
    }
    pub fn instance() -> Self {
        Self::new(crate::paths::data_dir())
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn definition(id: &str) -> Result<&'static PromptDefinition, String> {
        definitions()
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "未知系统提示词".into())
    }
    pub fn path(&self, id: &str) -> Result<PathBuf, String> {
        Self::definition(id)?;
        Ok(self.directory.join(format!("{id}.md")))
    }
    fn regular(path: &Path, directory: bool) -> Result<(), String> {
        match fs::symlink_metadata(path) {
            Ok(meta)
                if meta.file_type().is_symlink()
                    || (directory && !meta.is_dir())
                    || (!directory && !meta.is_file()) =>
            {
                Err(format!(
                    "提示词路径必须是普通{}，不能使用符号链接：{}",
                    if directory { "目录" } else { "文件" },
                    path.display()
                ))
            }
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn validate(text: &str) -> Result<(), String> {
        if text.trim().is_empty() || text.len() as u64 > MAX_BYTES || text.contains('\0') {
            return Err("提示词不能为空，不能包含 NUL，且不能超过 128 KiB".into());
        }
        Ok(())
    }
    /// Startup validation is read-only: missing files inherit this release's defaults.
    pub fn validate_all(&self) -> Result<(), String> {
        Self::regular(&self.directory, true)?;
        for entry in definitions() {
            self.read(entry.id)?;
        }
        Ok(())
    }
    pub fn overridden(&self, id: &str) -> Result<bool, String> {
        let path = self.path(id)?;
        Self::regular(&self.directory, true)?;
        Self::regular(&path, false)?;
        path.try_exists().map_err(|e| e.to_string())
    }
    pub fn read(&self, id: &str) -> Result<String, String> {
        let path = self.path(id)?;
        Self::regular(&self.directory, true)?;
        Self::regular(&path, false)?;
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let text = match options.open(&path) {
            Ok(file) => {
                let mut text = String::new();
                file.take(MAX_BYTES + 1)
                    .read_to_string(&mut text)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                text
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Self::definition(id)?.default.to_owned()
            }
            Err(e) => return Err(e.to_string()),
        };
        Self::validate(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(text)
    }
    pub fn save(&self, id: &str, text: &str, expected: &str) -> Result<(), String> {
        Self::validate(text)?;
        let _lock = WRITES.lock().map_err(|e| e.to_string())?;
        if self.read(id)? != expected {
            return Err("提示词已被修改，请重新加载后再保存".into());
        }
        let path = self.path(id)?;
        fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        let mut temp =
            tempfile::NamedTempFile::new_in(&self.directory).map_err(|e| e.to_string())?;
        temp.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn reset(&self, id: &str, expected: &str) -> Result<(), String> {
        let _lock = WRITES.lock().map_err(|e| e.to_string())?;
        if self.read(id)? != expected {
            return Err("提示词已被修改，请重新加载后再保存".into());
        }
        match fs::remove_file(self.path(id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherits_without_copying_and_preserves_isolated_overrides_until_reset() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let a = PromptStore::new(a.path());
        let b = PromptStore::new(b.path());
        a.validate_all().unwrap();
        b.validate_all().unwrap();
        assert!(!a.directory().exists());
        assert!(!b.directory().exists());
        let original = a.read("discussion").unwrap();
        a.save("discussion", "custom discussion", &original)
            .unwrap();
        a.validate_all().unwrap();
        assert_eq!(fs::read_dir(a.directory()).unwrap().count(), 1);
        assert!(a.overridden("discussion").unwrap());
        assert!(!b.overridden("discussion").unwrap());
        assert_eq!(a.read("discussion").unwrap(), "custom discussion");
        assert_eq!(b.read("discussion").unwrap(), original);
        fs::write(a.path("discussion").unwrap(), "editor change").unwrap();
        assert_eq!(a.read("discussion").unwrap(), "editor change");
        assert!(a.save("discussion", "stale", "custom discussion").is_err());
        assert!(a.save("discussion", "", "editor change").is_err());
        assert!(a.read("../secret").is_err());
        assert!(a.reset("discussion", "stale").is_err());
        a.reset("discussion", "editor change").unwrap();
        assert!(!a.path("discussion").unwrap().exists());
        assert_eq!(a.read("discussion").unwrap(), original);
        a.validate_all().unwrap();
        assert!(!a.path("discussion").unwrap().exists());
        // Saving the default text explicitly is still an override, not inheritance.
        a.save("discussion", &original, &original).unwrap();
        assert!(a.overridden("discussion").unwrap());
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_redirect_initialization_or_saving() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("conf")).unwrap();
        assert!(PromptStore::new(root.path()).validate_all().is_err());
        assert!(!outside.path().join("agent.md").exists());
    }
}
