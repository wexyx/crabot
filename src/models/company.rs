use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// 公司结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: Uuid,
    pub name: String,
    pub ceo_id: Option<Uuid>,
    pub total_employees: u32,
    pub total_departments: u32,
    pub founded_at: DateTime<Utc>,
    pub description: String,
}

impl Company {
    pub fn new(name: String, description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            ceo_id: None,
            total_employees: 0,
            total_departments: 0,
            founded_at: Utc::now(),
            description,
        }
    }

    /// 设置CEO
    pub fn set_ceo(&mut self, ceo_id: Uuid) {
        self.ceo_id = Some(ceo_id);
    }

    /// 添加员工
    pub fn add_employee(&mut self) {
        self.total_employees += 1;
    }

    /// 添加部门
    pub fn add_department(&mut self) {
        self.total_departments += 1;
    }

    /// 获取公司规模描述
    pub fn get_size_description(&self) -> String {
        match self.total_employees {
            0..=50 => "小型".to_string(),
            51..=200 => "中型".to_string(),
            201..=1000 => "大型".to_string(),
            _ => "超大型".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_company_creation() {
        let company = Company::new(
            "字节跳动".to_string(),
            "一家创意工程公司".to_string(),
        );

        assert_eq!(company.name, "字节跳动");
        assert_eq!(company.total_employees, 0);
    }

    #[test]
    fn test_company_size_description() {
        let mut company = Company::new(
            "test".to_string(),
            "test".to_string(),
        );

        assert_eq!(company.get_size_description(), "小型");

        company.total_employees = 100;
        assert_eq!(company.get_size_description(), "中型");

        company.total_employees = 1000;
        assert_eq!(company.get_size_description(), "大型");
    }
}
