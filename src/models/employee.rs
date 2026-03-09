use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::Skill;
use rbatis::crud_table;

/// 员工职位角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[crud_table]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub department_id: Option<String>, // CEO没有部门
    pub supervisor_id: Option<String>, // 上级ID
    pub hire_date: Option<DateTime<Utc>>,
    pub promotion_count: Option<u32>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
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
            id: Some(Uuid::new_v4().to_string()),
            name: Some(name),
            email: Some(email),
            role: Some("staff".to_string()),
            department_id: department_id.map(|id| id.to_string()),
            supervisor_id: None,
            hire_date: Some(Utc::now()),
            promotion_count: Some(0),
            is_active: Some(true),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        }
    }

    /// 获取角色枚举
    pub fn get_role_enum(&self) -> EmployeeRole {
        self.role.as_ref().map(|r| EmployeeRole::from(r.clone())).unwrap_or(EmployeeRole::Staff)
    }

    /// 设置角色
    pub fn set_role(&mut self, role: EmployeeRole) {
        self.role = Some(role.to_string());
    }
}
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
