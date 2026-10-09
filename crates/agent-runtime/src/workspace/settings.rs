use super::OutsideAccess;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Per-execution settings, never process-global environment mutations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSettings {
    #[serde(default)]
    workdir: Option<PathBuf>,
    #[serde(default)]
    outside_access: Option<String>,
}

tokio::task_local! { static SETTINGS: WorkspaceSettings; }

impl WorkspaceSettings {
    pub fn workdir(&self) -> Option<&std::path::Path> {
        self.workdir.as_deref()
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(path) = &self.workdir {
            if !path.is_absolute() || !path.is_dir() {
                return Err("项目工作目录必须是当前节点上已存在的绝对目录".into());
            }
        }
        if !matches!(self.outside_access.as_deref(), None | Some("deny" | "ask")) {
            return Err("outside_access must be deny or ask".into());
        }
        if self.outside_access.as_deref() == Some("ask")
            && OutsideAccess::from_env()? == OutsideAccess::Deny
        {
            return Err("实例禁止目录外访问；请以 --outside-access ask 启动后再配置询问".into());
        }
        Ok(())
    }

    pub async fn scope<T>(self, future: impl std::future::Future<Output = T>) -> T {
        SETTINGS.scope(self, future).await
    }

    pub(crate) fn root() -> Option<PathBuf> {
        SETTINGS
            .try_with(|settings| settings.workdir.clone())
            .ok()
            .flatten()
    }

    pub(crate) fn outside() -> Option<OutsideAccess> {
        SETTINGS
            .try_with(|settings| match settings.outside_access.as_deref() {
                Some("deny") => Some(OutsideAccess::Deny),
                Some("ask") => Some(OutsideAccess::Ask),
                _ => None,
            })
            .ok()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn concurrent_scopes_do_not_leak() {
        let a = WorkspaceSettings {
            workdir: Some("/first".into()),
            outside_access: Some("deny".into()),
        };
        let b = WorkspaceSettings {
            workdir: Some("/second".into()),
            outside_access: None,
        };
        let (first, second) = tokio::join!(
            a.scope(async {
                tokio::task::yield_now().await;
                WorkspaceSettings::root()
            }),
            b.scope(async { WorkspaceSettings::root() })
        );
        assert_eq!(first, Some("/first".into()));
        assert_eq!(second, Some("/second".into()));
        assert!(WorkspaceSettings::root().is_none());
    }
    #[test]
    fn rejects_relative_missing_and_unknown_settings() {
        for input in [
            serde_json::json!({"workdir":"."}),
            serde_json::json!({"workdir":"/nonexistent-crabot-fixture"}),
            serde_json::json!({"outside_access":"allow"}),
        ] {
            assert!(
                serde_json::from_value::<WorkspaceSettings>(input)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
    }
}
