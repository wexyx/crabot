use super::approvals;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{decide, pending};
    #[tokio::test]
    async fn outside_path_requires_explicit_one_shot_approval() {
        let base = std::env::temp_dir().join(format!("agent-workspace-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(base.join("root")).unwrap();
        std::fs::write(base.join("outside.txt"), "outside").unwrap();
        let denied = Workspace::new(base.join("root"), OutsideAccess::Deny).unwrap();
        assert!(
            denied
                .authorize(Path::new("../outside.txt"), "read")
                .await
                .is_err()
        );
        let workspace = Workspace::new(base.join("root"), OutsideAccess::Ask).unwrap();
        let task = tokio::spawn(async move {
            workspace
                .authorize(Path::new("../outside.txt"), "read")
                .await
        });
        let expected = base
            .join("outside.txt")
            .canonicalize()
            .unwrap()
            .display()
            .to_string();
        let mut request = None;
        for _ in 0..100 {
            request = pending().into_iter().find(|r| r.path == expected);
            if request.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let request = request.unwrap();
        decide(request.id, true).unwrap();
        assert_eq!(task.await.unwrap().unwrap().display().to_string(), expected);
        assert!(decide(request.id, true).is_err());
        assert!(!pending().iter().any(|r| r.id == request.id));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[tokio::test]
    async fn symlink_outside_is_not_an_inside_path() {
        #[cfg(unix)]
        {
            let base =
                std::env::temp_dir().join(format!("agent-workspace-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(base.join("root")).unwrap();
            std::fs::write(base.join("secret"), "outside").unwrap();
            std::os::unix::fs::symlink(base.join("secret"), base.join("root/link")).unwrap();
            let workspace = Workspace::new(base.join("root"), OutsideAccess::Deny).unwrap();
            assert!(
                workspace
                    .authorize(Path::new("link"), "read")
                    .await
                    .is_err()
            );
            std::fs::remove_dir_all(base).unwrap();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutsideAccess {
    Deny,
    Ask,
}
impl OutsideAccess {
    pub fn from_env() -> Result<Self, String> {
        if let Some(value) = super::WorkspaceSettings::outside() {
            return Ok(value);
        }
        match std::env::var("AGENT_OUTSIDE_ACCESS")
            .unwrap_or_else(|_| "deny".into())
            .as_str()
        {
            "deny" => Ok(Self::Deny),
            "ask" => Ok(Self::Ask),
            _ => Err("AGENT_OUTSIDE_ACCESS must be deny or ask".into()),
        }
    }
}
#[derive(Clone)]
pub struct Workspace {
    root: PathBuf,
    outside: OutsideAccess,
}
impl Workspace {
    pub fn new(root: PathBuf, outside: OutsideAccess) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("invalid workdir: {e}"))?;
        if !root.is_dir() {
            return Err("workdir must be a directory".into());
        }
        Ok(Self { root, outside })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub async fn authorize(&self, path: &Path, operation: &str) -> Result<PathBuf, String> {
        let target = self
            .root
            .join(path)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !target.starts_with(&self.root)
            && crate::permissions::PermissionMode::current()
                != crate::permissions::PermissionMode::Full
        {
            match self.outside {
                OutsideAccess::Deny => {
                    return Err("outside-directory access denied by startup policy".into());
                }
                OutsideAccess::Ask => approvals::request(&self.root, &target, operation).await?,
            }
        }
        if self
            .root
            .join(path)
            .canonicalize()
            .map_err(|e| e.to_string())?
            != target
        {
            return Err("path changed while awaiting approval; retry".into());
        }
        Ok(target)
    }
}
