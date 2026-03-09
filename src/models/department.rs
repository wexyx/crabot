use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::Employee;

/// 部门类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
pub enum DepartmentType {
    #[serde(rename = "hr")]
    HR = 1,
    
    #[serde(rename = "sales")]
    Sales = 2,
    
    #[serde(rename = "engineering")]
    Engineering = 3,
    
    #[serde(rename = "operations")]
    Operations = 4,
    
    #[serde(rename = "marketing")]
    Marketing = 5,
}

impl ToString for DepartmentType {
    fn to_string(&self) -> String {
        match self {
            DepartmentType::HR => "hr".to_string(),
            DepartmentType::Sales => "sales".to_string(),
            DepartmentType::Engineering => "engineering".to_string(),
            DepartmentType::Operations => "operations".to_string(),
            DepartmentType::Marketing => "marketing".to_string(),
        }
    }
}

impl From<String> for DepartmentType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "hr" => DepartmentType::HR,
            "sales" => DepartmentType::Sales,
            "engineering" => DepartmentType::Engineering,
            "operations" => DepartmentType::Operations,
            "marketing" => DepartmentType::Marketing,
            _ => DepartmentType::Operations,
        }
    }
}

/// 部门结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Department {
    pub id: Uuid,
    pub name: String,
    pub dept_type: DepartmentType,
    pub leader_id: Option<Uuid>, // 部门leader
    pub parent_dept_id: Option<Uuid>, // 父部门ID
    pub employee_count: u32,
    pub created_at: DateTime<Utc>,
    pub description: String,
}

impl Department {
    pub fn new(name: String, dept_type: DepartmentType, description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            dept_type,
            leader_id: None,
            parent_dept_id: None,
            employee_count: 0,
            created_at: Utc::now(),
            description,
        }
    }

    /// 设置部门leader
    pub fn set_leader(&mut self, leader_id: Uuid) {
        self.leader_id = Some(leader_id);
    }

    /// 添加员工
    pub fn add_employee(&mut self) {
        self.employee_count += 1;
    }

    /// 移除员工
    pub fn remove_employee(&mut self) {
        if self.employee_count > 0 {
            self.employee_count -= 1;
        }
    }

    /// 获取部门的关键技能需求
    pub fn get_required_skills(&self) -> Vec<(String, crate::models::SkillLevel)> {
        match self.dept_type {
            DepartmentType::HR => vec![
                ("招聘".to_string(), crate::models::SkillLevel::Intermediate),
                ("员工管理".to_string(), crate::models::SkillLevel::Intermediate),
            ],
            DepartmentType::Sales => vec![
                ("销售".to_string(), crate::models::SkillLevel::Intermediate),
                ("沟通".to_string(), crate::models::SkillLevel::Advanced),
            ],
            DepartmentType::Engineering => vec![
                ("编程".to_string(), crate::models::SkillLevel::Advanced),
                ("系统设计".to_string(), crate::models::SkillLevel::Intermediate),
            ],
            DepartmentType::Operations => vec![
                ("流程管理".to_string(), crate::models::SkillLevel::Intermediate),
                ("数据分析".to_string(), crate::models::SkillLevel::Beginner),
            ],
            DepartmentType::Marketing => vec![
                ("市场分析".to_string(), crate::models::SkillLevel::Intermediate),
                ("创意".to_string(), crate::models::SkillLevel::Intermediate),
            ],
        }
    }

    /// 检查员工是否满足部门要求
    pub fn validate_employee(&self, employee: &Employee) -> bool {
        employee.has_required_skills(&self.get_required_skills())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_department_creation() {
        let dept = Department::new(
            "销售部".to_string(),
            DepartmentType::Sales,
            "负责产品销售".to_string(),
        );

        assert_eq!(dept.name, "销售部");
        assert_eq!(dept.dept_type, DepartmentType::Sales);
        assert_eq!(dept.employee_count, 0);
    }

    #[test]
    fn test_department_employee_count() {
        let mut dept = Department::new(
            "工程部".to_string(),
            DepartmentType::Engineering,
            "负责产品开发".to_string(),
        );

        dept.add_employee();
        dept.add_employee();
        assert_eq!(dept.employee_count, 2);

        dept.remove_employee();
        assert_eq!(dept.employee_count, 1);
    }
}
