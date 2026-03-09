use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 技能等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
pub enum SkillLevel {
    #[serde(rename = "beginner")]
    Beginner = 1,
    
    #[serde(rename = "intermediate")]
    Intermediate = 2,
    
    #[serde(rename = "advanced")]
    Advanced = 3,
    
    #[serde(rename = "expert")]
    Expert = 4,
}

impl ToString for SkillLevel {
    fn to_string(&self) -> String {
        match self {
            SkillLevel::Beginner => "beginner".to_string(),
            SkillLevel::Intermediate => "intermediate".to_string(),
            SkillLevel::Advanced => "advanced".to_string(),
            SkillLevel::Expert => "expert".to_string(),
        }
    }
}

impl From<String> for SkillLevel {
    fn from(s: String) -> Self {
        match s.as_str() {
            "beginner" => SkillLevel::Beginner,
            "intermediate" => SkillLevel::Intermediate,
            "advanced" => SkillLevel::Advanced,
            "expert" => SkillLevel::Expert,
            _ => SkillLevel::Beginner,
        }
    }
}

/// 技能定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub level: SkillLevel,
    pub category: String, // 比如：销售、管理、技术、沟通等
}

impl Skill {
    pub fn new(name: String, description: String, level: SkillLevel, category: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            level,
            category,
        }
    }

    /// 检查技能是否满足最低要求
    pub fn meets_requirement(&self, required_level: SkillLevel) -> bool {
        self.level as u8 >= required_level as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_meets_requirement() {
        let skill = Skill::new(
            "Sales".to_string(),
            "Sales skill".to_string(),
            SkillLevel::Advanced,
            "Sales".to_string(),
        );

        assert!(skill.meets_requirement(SkillLevel::Beginner));
        assert!(skill.meets_requirement(SkillLevel::Advanced));
        assert!(!skill.meets_requirement(SkillLevel::Expert));
    }
}
