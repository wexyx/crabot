/// Operator-controlled policy for the in-process execution supervisor.
#[derive(Clone)]
pub struct ExecutionPolicy {
    profile: String,
    allowed_profiles: Vec<String>,
}
impl ExecutionPolicy {
    pub fn new(profile: String) -> Result<Self, String> {
        if profile.is_empty() {
            return Err("execution profile required".into());
        }
        Ok(Self {
            allowed_profiles: vec![profile.clone()],
            profile,
        })
    }
    pub fn from_env() -> Result<Self, String> {
        let policy = Self::new(
            std::env::var("CRABOT_EXECUTION_PROFILE")
                .or_else(|_| std::env::var("CRABOT_SANDBOX_PROFILE"))
                .unwrap_or_else(|_| "default".into()),
        )?;
        if let Ok(value) = std::env::var("CRABOT_EXECUTION_ALLOWED_PROFILES")
            .or_else(|_| std::env::var("CRABOT_SANDBOX_ALLOWED_PROFILES"))
        {
            if !value.trim().is_empty() {
                return policy.with_allowed_profiles(
                    value.split(',').map(|v| v.trim().to_owned()).collect(),
                );
            }
        }
        Ok(policy)
    }
    pub fn with_allowed_profiles(mut self, profiles: Vec<String>) -> Result<Self, String> {
        if profiles.is_empty()
            || profiles.len() > 16
            || !profiles.contains(&self.profile)
            || profiles.iter().any(|p| {
                p.is_empty()
                    || p.len() > 64
                    || !p
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            })
        {
            return Err(
                "invalid execution profile allowlist; it must include the default profile".into(),
            );
        }
        self.allowed_profiles = profiles;
        Ok(self)
    }
    pub(crate) fn select_profile(&self, requested: Option<&str>) -> Result<String, String> {
        let selected = requested.unwrap_or(&self.profile);
        if !self.allowed_profiles.iter().any(|p| p == selected) {
            return Err("skill execution profile is not allowed by this Agent".into());
        }
        Ok(selected.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skill_can_only_select_operator_allowed_profiles() {
        let policy = ExecutionPolicy::new("offline".into())
            .unwrap()
            .with_allowed_profiles(vec!["offline".into(), "business".into()])
            .unwrap();
        assert_eq!(policy.select_profile(None).unwrap(), "offline");
        assert_eq!(policy.select_profile(Some("business")).unwrap(), "business");
        assert!(policy.select_profile(Some("privileged")).is_err());
        assert!(ExecutionPolicy::new("".into()).is_err());
    }
}
