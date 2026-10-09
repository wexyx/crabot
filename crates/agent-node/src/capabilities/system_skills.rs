use super::Resource;
use agent_runtime::skills::SkillDefinition;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

fn directory() -> PathBuf {
    std::env::var_os("CRABOT_SYSTEM_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let bundled = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent()?.parent().map(|p| p.join("skills/system")));
            bundled.filter(|p| p.is_dir()).unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/system")
            })
        })
}

pub(super) async fn resources(scope: &str) -> Result<Vec<Resource>, String> {
    let root = directory();
    let scope = scope.to_owned();
    tokio::task::spawn_blocking(move || read(&root, &scope))
        .await
        .map_err(|e| e.to_string())?
}

fn text(path: &Path) -> Result<String, String> {
    if !std::fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_file()
    {
        return Err(format!(
            "Skill files must be regular files: {}",
            path.display()
        ));
    }
    let mut content = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_string(&mut content)
        .map_err(|e| e.to_string())?;
    if content.len() > 65536 {
        return Err("Skill file exceeds 64 KiB".into());
    }
    Ok(content)
}

fn files(
    root: &Path,
    relative: &Path,
    result: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(root.join(relative)).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Skill filename must be UTF-8")?;
        if name.starts_with('.') || (relative.as_os_str().is_empty() && name == "skill.json") {
            continue;
        }
        let path = relative.join(name);
        if path.components().count() > 8 {
            return Err("Skill directory nesting exceeds 8".into());
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            files(root, &path, result)?;
        } else if kind.is_file() {
            if result.len() >= 32 {
                return Err("Skill exceeds 32 files".into());
            }
            result.insert(
                path.to_string_lossy().replace('\\', "/"),
                text(&entry.path())?,
            );
        } else {
            return Err("Skill symlinks and special files are not allowed".into());
        }
    }
    Ok(())
}

fn read(root: &Path, scope: &str) -> Result<Vec<Resource>, String> {
    let scope_dir = root.join(scope);
    let entries = match std::fs::read_dir(&scope_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    let mut resources = vec![];
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        if resources.len() >= 32 {
            return Err("System Skills exceed 32 per scope".into());
        }
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| "invalid Skill name")?;
        if id.starts_with('.') {
            continue;
        }
        let metadata: Value = serde_json::from_str(&text(&entry.path().join("skill.json"))?)
            .map_err(|e| e.to_string())?;
        let mut contents = BTreeMap::new();
        files(&entry.path(), Path::new(""), &mut contents)?;
        // Packaged content does not grant arbitrary Python execution.
        let definition = SkillDefinition::new(
            id.clone(),
            metadata["description"]
                .as_str()
                .ok_or("Skill description missing")?
                .into(),
            contents,
            metadata["enabled"] == true,
            false,
        )?;
        resources.push(Resource {
            id: format!("builtin:{scope}:skill:{id}"),
            kind: "skill".into(),
            scope: scope.into(),
            definition: serde_json::to_value(definition).map_err(|e| e.to_string())?,
            version: 0,
            readonly: true,
            origin: None,
        });
    }
    Ok(resources)
}

pub(super) fn fallback_management_guide() -> Resource {
    Resource {
        id: "builtin:management:skill:management-guide".into(),
        kind: "skill".into(),
        scope: "management".into(),
        definition: json!({"id":"management-guide","description":"内置管理规则","enabled":true,"allow_python":false,"files":{"SKILL.md":include_str!("../../../../skills/system/management/management-guide/SKILL.md")}}),
        version: 0,
        readonly: true,
        origin: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_skills_load_as_readonly_and_never_grant_python() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/system");
        let rows = read(&root, "business").unwrap();
        assert_eq!(rows.len(), 5);
        for row in rows {
            assert!(row.readonly);
            assert_eq!(row.definition["allow_python"], false);
            assert!(row.definition["files"]["SKILL.md"].is_string());
        }
        assert_eq!(
            read(&root, "management").unwrap()[0].id,
            fallback_management_guide().id
        );
    }
    #[test]
    fn symlink_scripts_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let skill = dir.path().join("business/test");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("skill.json"), r#"{"description":"test"}"#).unwrap();
        std::fs::write(skill.join("SKILL.md"), "test").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("SKILL.md", skill.join("linked.md")).unwrap();
            assert!(read(dir.path(), "business").is_err());
        }
    }
}
