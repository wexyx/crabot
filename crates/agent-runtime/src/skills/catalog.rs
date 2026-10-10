use super::SkillDefinition;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub struct SkillCatalog {
    skills: BTreeMap<String, SkillDefinition>,
    builtins: BTreeSet<String>,
}
impl SkillCatalog {
    pub fn new(skills: Vec<SkillDefinition>) -> Result<Self, String> {
        if skills.len() > 32
            || serde_json::to_vec(&skills)
                .map_err(|e| e.to_string())?
                .len()
                > 1024 * 1024
        {
            return Err("project skill snapshot exceeds 32 skills or 1 MiB".into());
        }
        let mut result = Self::default();
        for skill in skills {
            skill.validate()?;
            if skill.enabled() && result.skills.insert(skill.id().into(), skill).is_some() {
                return Err("duplicate skill id".into());
            }
        }
        Ok(result)
    }
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
    pub fn definitions(&self) -> Vec<SkillDefinition> {
        self.skills.values().cloned().collect()
    }
    /// Origin is host metadata, never a field a user package can set.
    pub fn with_builtins(mut self, ids: impl IntoIterator<Item = String>) -> Self {
        self.builtins = ids
            .into_iter()
            .filter(|id| self.skills.contains_key(id))
            .collect();
        self
    }
    pub(crate) fn is_builtin(&self, id: &str) -> bool {
        self.builtins.contains(id)
    }
    pub(crate) fn get(&self, id: &str) -> Result<&SkillDefinition, String> {
        self.skills
            .get(id)
            .ok_or_else(|| "unknown or disabled skill".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_skills_cannot_be_discovered_or_read() {
        let skill = SkillDefinition::new(
            "disabled".into(),
            "hidden".into(),
            BTreeMap::from([("SKILL.md".into(), "private instructions".into())]),
            false,
            false,
        )
        .unwrap();
        let catalog = SkillCatalog::new(vec![skill]).unwrap();
        assert!(catalog.is_empty());
        assert!(catalog.get("disabled").is_err());
        assert!(!catalog.definitions().iter().any(|s| s.id() == "disabled"));
    }
}
