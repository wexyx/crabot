use rbatis::rbatis::Rbatis;
use crate::{Result, CrabotError};
use crate::models::Skill;

/// 技能数据库操作
pub struct SkillDatabase<'a> {
    rb: &'a Rbatis,
}

impl<'a> SkillDatabase<'a> {
    pub fn new(rb: &'a Rbatis) -> Self {
        Self { rb }
    }

    /// 创建技能
    pub async fn create_skill(&self, skill: &Skill) -> Result<()> {
        self.rb.save("", skill).await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取技能信息
    pub async fn get_skill(&self, skill_id: &str) -> Result<Option<Skill>> {
        let result: Option<Skill> = self.rb.fetch_by_column("", "id", skill_id).await
            .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(result)
    }
}
            }
        }))
    }
}