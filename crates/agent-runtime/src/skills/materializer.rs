use super::SkillDefinition;
use std::{collections::HashMap, path::PathBuf};
use tokio::sync::Mutex;

/// Per-registry execution copies of enabled Skill snapshots. Package sources and
/// instance configuration are never modified; scripts still require shell approval.
pub(crate) struct SkillMaterializer {
    workdir: Option<PathBuf>,
    directories: Mutex<HashMap<String, PathBuf>>,
}
impl SkillMaterializer {
    pub(crate) fn new(workdir: Option<PathBuf>) -> Self {
        Self {
            workdir,
            directories: Mutex::new(HashMap::new()),
        }
    }
    pub(crate) async fn directory(
        &self,
        skill: &SkillDefinition,
    ) -> Result<Option<PathBuf>, String> {
        let Some(root) = &self.workdir else {
            return Ok(None);
        };
        let mut directories = self.directories.lock().await;
        if let Some(path) = directories.get(skill.id()) {
            return Ok(Some(path.clone()));
        }
        let root = root.clone();
        let package = skill.clone();
        let path = tokio::task::spawn_blocking(move || {
            package.validate()?;
            if !package.enabled() {
                return Err("disabled skill".to_owned());
            }
            let dir = tempfile::Builder::new()
                .prefix("skill-")
                .tempdir_in(crate::workspace::temporary_dir(&root)?)
                .map_err(|e| e.to_string())?;
            for (name, content) in package.files() {
                let path = dir.path().join(name);
                std::fs::create_dir_all(path.parent().ok_or("invalid skill path")?)
                    .map_err(|e| e.to_string())?;
                std::fs::write(&path, content).map_err(|e| e.to_string())?;
            }
            // Retained in the workspace tmp area, so subsequent shell calls and
            // generated scripts can use imports even after the current tool returns.
            Ok::<_, String>(dir.keep())
        })
        .await
        .map_err(|e| e.to_string())??;
        directories.insert(skill.id().into(), path.clone());
        Ok(Some(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[tokio::test]
    async fn packages_are_complete_cached_and_instance_local() {
        let root = tempfile::tempdir().unwrap();
        let skill = SkillDefinition::new(
            "demo".into(),
            "test".into(),
            BTreeMap::from([
                ("SKILL.md".into(), "instructions".into()),
                ("scripts/main.py".into(), "print(42)".into()),
            ]),
            true,
            false,
        )
        .unwrap();
        let materializer = SkillMaterializer::new(Some(root.path().into()));
        let first = materializer.directory(&skill).await.unwrap().unwrap();
        assert!(first.starts_with(crate::workspace::temporary_dir(root.path()).unwrap()));
        assert_eq!(
            std::fs::read_to_string(first.join("scripts/main.py")).unwrap(),
            "print(42)"
        );
        assert_eq!(materializer.directory(&skill).await.unwrap(), Some(first));
        assert!(
            SkillMaterializer::new(None)
                .directory(&skill)
                .await
                .unwrap()
                .is_none()
        );
    }
}
