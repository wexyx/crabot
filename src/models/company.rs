use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use rbatis::crud_table;
use rbatis::rbatis::Rbatis;

/// 公司结构
#[crud_table]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: Option<String>,
    pub name: Option<String>,
    pub ceo_id: Option<String>,
    pub total_employees: Option<u32>,
    pub total_departments: Option<u32>,
    pub founded_at: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Company {
    pub fn new(name: String, description: String) -> Self {
        Self {
            id: Some(Uuid::new_v4().to_string()),
            name: Some(name),
            ceo_id: None,
            total_employees: Some(0),
            total_departments: Some(0),
            founded_at: Some(Utc::now()),
            description: Some(description),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        }
    }

    /// 设置CEO
    pub fn set_ceo(&mut self, ceo_id: Uuid) {
        self.ceo_id = Some(ceo_id.to_string());
    }

    /// 添加员工
    pub fn add_employee(&mut self) {
        if let Some(ref mut count) = self.total_employees {
            *count += 1;
        }
    }

    /// 添加部门
    pub fn add_department(&mut self) {
        if let Some(ref mut count) = self.total_departments {
            *count += 1;
        }
    }

    /// 获取公司规模描述
    pub fn get_size_description(&self) -> String {
        let count = self.total_employees.unwrap_or(0);
        match count {
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
