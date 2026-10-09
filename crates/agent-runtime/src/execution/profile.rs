use serde::Deserialize;
use std::collections::BTreeMap;
/// Host-owned native execution policy, never supplied by a model.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    pub id: String,
    pub network: String,
    pub timeout_seconds: u64,
    #[serde(default)]
    pub secret_env: Vec<String>,
}
impl Profile {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err("invalid profile id".into());
        }
        if self.network != "host" {
            return Err(
                "network must be host; Crabot no longer isolates process networking".into(),
            );
        }
        if !(1..=120).contains(&self.timeout_seconds) {
            return Err("timeout_seconds must be 1..120".into());
        }
        if self.secret_env.len() > 16
            || self.secret_env.iter().any(|k| {
                k.is_empty()
                    || k.len() > 64
                    || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    || k.starts_with("PYTHON")
                    || k.starts_with("LD_")
                    || k.starts_with("DYLD_")
                    || matches!(k.as_str(), "PATH" | "HOME" | "TMPDIR" | "SHELL")
            })
        {
            return Err("invalid secret environment selection".into());
        }
        Ok(())
    }
    pub(crate) fn secrets(&self) -> Result<BTreeMap<String, String>, String> {
        self.secret_env
            .iter()
            .map(|k| {
                Ok((
                    k.clone(),
                    std::env::var(k)
                        .map_err(|_| format!("required Agent secret {k} is missing"))?,
                ))
            })
            .collect()
    }
}
