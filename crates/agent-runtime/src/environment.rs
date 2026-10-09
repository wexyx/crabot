use std::{collections::BTreeMap, ffi::OsString};

tokio::task_local! { static CURRENT: AgentEnvironment; }

/// Explicit per-Agent variables. Never mutate the process-wide environment.
#[derive(Clone, Default)]
pub struct AgentEnvironment {
    values: BTreeMap<String, String>,
}
impl AgentEnvironment {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let values: BTreeMap<String, String> = if raw.trim().is_empty() {
            BTreeMap::new()
        } else {
            serde_json::from_str(raw).map_err(|_| "环境变量必须是 JSON 键值对象，值必须是字符串")?
        };
        if values.len() > 64 || raw.len() > 65536 {
            return Err("最多配置 64 个环境变量，总长度不超过 64 KiB".into());
        }
        for (key, value) in &values {
            if key.is_empty()
                || key.len() > 128
                || !key.bytes().enumerate().all(|(i, c)| {
                    c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                return Err("环境变量名称仅支持字母、数字和下划线，不能以数字开头".into());
            }
            if value.contains('\0') || value.len() > 8192 {
                return Err("环境变量值不可包含 NUL，且不能超过 8 KiB".into());
            }
            if key.starts_with("CRABOT_")
                || key.starts_with("AGENT_")
                || matches!(
                    key.as_str(),
                    "HOME"
                        | "USERPROFILE"
                        | "TMPDIR"
                        | "TMP"
                        | "TEMP"
                        | "CODEX_HOME"
                        | "MAC_CHROMIUM_TMPDIR"
                        | "CFFIXED_USER_HOME"
                        | "XDG_CONFIG_HOME"
                        | "XDG_CACHE_HOME"
                        | "PUPPETEER_CACHE_DIR"
                )
            {
                return Err(format!("环境变量 {key} 由 Crabot 管理，不允许覆盖"));
            }
        }
        Ok(Self { values })
    }
    pub fn get(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.values).expect("string map")
    }
    pub fn masked_json(&self) -> String {
        serde_json::to_string(
            &self
                .values
                .keys()
                .map(|k| (k, None::<String>))
                .collect::<BTreeMap<_, _>>(),
        )
        .expect("string map")
    }
    /// In an edit, null preserves an existing value; an omitted name deletes it.
    pub fn merge(current: &str, update: &str) -> Result<Self, String> {
        let previous = Self::from_json(current)?;
        let patch: BTreeMap<String, Option<String>> = if update.trim().is_empty() {
            BTreeMap::new()
        } else {
            serde_json::from_str(update).map_err(|_| "环境变量必须是 JSON 键值对象")?
        };
        let mut next = BTreeMap::new();
        for (key, value) in patch {
            let value = value
                .or_else(|| previous.get(&key).cloned())
                .ok_or("新增环境变量需要填写值")?;
            next.insert(key, value);
        }
        Self::from_json(&serde_json::to_string(&next).map_err(|_| "invalid environment")?)
    }
    pub async fn scope<F: std::future::Future>(&self, future: F) -> F::Output {
        CURRENT.scope(self.clone(), future).await
    }
    pub(crate) fn lookup(key: &str) -> Option<OsString> {
        CURRENT
            .try_with(|env| env.get(key).map(OsString::from))
            .ok()
            .flatten()
            .or_else(|| std::env::var_os(key))
    }
    pub(crate) fn apply(command: &mut tokio::process::Command) {
        for key in ["LC_ALL", "LC_CTYPE"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        let _ = CURRENT.try_with(|env| {
            command.envs(&env.values);
        });
    }
    /// Overlay these variables onto a command that otherwise inherits the host.
    ///
    /// For host introspection such as reading the OpenCode model catalog, which
    /// lives in the user's own OpenCode profile rather than in a workspace.
    pub fn overlay(&self, command: &mut tokio::process::Command) {
        command.envs(&self.values);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[tokio::test]
    async fn native_children_receive_only_their_scoped_environment() {
        async fn run(value: &str) -> String {
            let root = tempfile::tempdir().unwrap();
            let env = AgentEnvironment::from_json(
                &serde_json::json!({"TEST_AGENT_ENV":value}).to_string(),
            )
            .unwrap();
            env.scope(async {
                let launcher = crate::execution::native::NativeCommand::new(root.path()).unwrap();
                let output = launcher
                    .command(std::path::Path::new("/bin/sh"), root.path())
                    .unwrap()
                    .args(["-c", "printf '%s' \"$TEST_AGENT_ENV\""])
                    .output()
                    .await
                    .unwrap();
                String::from_utf8(output.stdout).unwrap()
            })
            .await
        }
        let (a, b) = tokio::join!(run("agent-a"), run("agent-b"));
        assert_eq!(a, "agent-a");
        assert_eq!(b, "agent-b");
    }
    #[tokio::test]
    async fn concurrent_scopes_do_not_leak_and_secrets_are_masked() {
        let a = AgentEnvironment::from_json(r#"{"TEST_AGENT_ENV":"first"}"#).unwrap();
        let b = AgentEnvironment::from_json(r#"{"TEST_AGENT_ENV":"second"}"#).unwrap();
        let (left, right) = tokio::join!(
            a.scope(async {
                tokio::task::yield_now().await;
                AgentEnvironment::lookup("TEST_AGENT_ENV")
            }),
            b.scope(async {
                tokio::task::yield_now().await;
                AgentEnvironment::lookup("TEST_AGENT_ENV")
            })
        );
        assert_eq!(left, Some("first".into()));
        assert_eq!(right, Some("second".into()));
        assert_eq!(std::env::var_os("TEST_AGENT_ENV"), None);
        assert_eq!(a.masked_json(), r#"{"TEST_AGENT_ENV":null}"#);
    }
    #[test]
    fn edit_preserves_updates_deletes_and_rejects_internal_overrides() {
        let changed = AgentEnvironment::merge(
            r#"{"TOKEN":"secret","DROP":"old"}"#,
            r#"{"TOKEN":null,"NEW":"value"}"#,
        )
        .unwrap();
        assert_eq!(changed.get("TOKEN").unwrap(), "secret");
        assert!(changed.get("DROP").is_none());
        for raw in [
            r#"{"HOME":"/tmp"}"#,
            r#"{"CRABOT_DATA_DIR":"/tmp"}"#,
            r#"{"1BAD":"x"}"#,
            r#"{"A":42}"#,
        ] {
            assert!(AgentEnvironment::from_json(raw).is_err());
        }
    }
}
