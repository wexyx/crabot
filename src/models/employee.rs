use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::Skill;

/// 员工职位角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
pub enum EmployeeRole {
    #[serde(rename = "staff")]
    Staff = 1,
    
    #[serde(rename = "leader")]
    Leader = 2,
    
    #[serde(rename = "ceo")]
    CEO = 3,
}

impl ToString for EmployeeRole {
    fn to_string(&self) -> String {
        match self {
            EmployeeRole::Staff => "staff".to_string(),
            EmployeeRole::Leader => "leader".to_string(),
            EmployeeRole::CEO => "ceo".to_string(),
        }
    }
}

impl From<String> for EmployeeRole {
    fn from(s: String) -> Self {
        match s.as_str() {
            "staff" => EmployeeRole::Staff,
            "leader" => EmployeeRole::Leader,
            "ceo" => EmployeeRole::CEO,
            _ => EmployeeRole::Staff,
        }
    }
}

/// 员工结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: EmployeeRole,
    pub department_id: Option<Uuid>, // CEO没有部门
    pub supervisor_id: Option<Uuid>, // 上级ID
    pub skills: Vec<Skill>,
    pub hire_date: DateTime<Utc>,
    pub promotion_count: u32,
    pub is_active: bool,
}

impl Employee {
    pub fn new(
        name: String,
        email: String,
        department_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            email,
            role: EmployeeRole::Staff,
            department_id,
            supervisor_id: None,
            skills: Vec::new(),
            hire_date: Utc::now(),
            promotion_count: 0,
            is_active: true,
        }
    }

    /// 添加技能
    pub fn add_skill(&mut self, skill: Skill) {
        // 检查是否已存在相同名称的技能
        if !self.skills.iter().any(|s| s.name == skill.name) {
            self.skills.push(skill);
        }
    }

    /// 升级技能等级
    pub fn upgrade_skill(&mut self, skill_name: &str, new_level: crate::models::SkillLevel) -> bool {
        if let Some(skill) = self.skills.iter_mut().find(|s| s.name == skill_name) {
            skill.level = new_level;
            true
        } else {
            false
        }
    }

    /// 获取指定技能
    pub fn get_skill(&self, skill_name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == skill_name)
    }

    /// 检查是否具有足够的技能
    pub fn has_required_skills(&self, required_skills: &[(String, crate::models::SkillLevel)]) -> bool {
        required_skills.iter().all(|(skill_name, required_level)| {
            self.get_skill(skill_name)
                .map(|skill| skill.meets_requirement(*required_level))
                .unwrap_or(false)
        })
    }

    /// 晋升为部门leader
    pub fn promote_to_leader(&mut self) -> bool {
        if self.role == EmployeeRole::Staff {
            self.role = EmployeeRole::Leader;
            self.promotion_count += 1;
            true
        } else {
            false
        }
    }

    /// 晋升为CEO
    pub fn promote_to_ceo(&mut self) -> bool {
        if self.role == EmployeeRole::Leader {
            self.role = EmployeeRole::CEO;
            self.promotion_count += 1;
            self.department_id = None; // CEO不属于任何部门
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_employee_skill_management() {
        let mut emp = Employee::new(
            "张三".to_string(),
            "zhangsan@bytedance.com".to_string(),
            None,
        );

        let skill = Skill::new(
            "Sales".to_string(),
            "Sales skill".to_string(),
            crate::models::SkillLevel::Intermediate,
            "Sales".to_string(),
        );

        emp.add_skill(skill);
        assert_eq!(emp.skills.len(), 1);
        assert!(emp.get_skill("Sales").is_some());
    }

    #[test]
    fn test_employee_promotion() {
        let mut emp = Employee::new(
            "李四".to_string(),
            "lisi@bytedance.com".to_string(),
            None,
        );

        assert_eq!(emp.role, EmployeeRole::Staff);
        assert!(emp.promote_to_leader());
        assert_eq!(emp.role, EmployeeRole::Leader);
        assert!(emp.promote_to_ceo());
        assert_eq!(emp.role, EmployeeRole::CEO);
    }
}
