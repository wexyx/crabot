use rbatis::rbatis::Rbatis;
use crate::{Result, CrabotError};
use crate::models::Employee;

/// 员工数据库操作
pub struct EmployeeDatabase<'a> {
    rb: &'a Rbatis,
}

impl<'a> EmployeeDatabase<'a> {
    pub fn new(rb: &'a Rbatis) -> Self {
        Self { rb }
    }

    /// 创建员工
    pub async fn create_employee(&self, employee: &Employee) -> Result<()> {
        self.rb.save("", employee).await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取员工信息
    pub async fn get_employee(&self, employee_id: &str) -> Result<Option<Employee>> {
        let result: Option<Employee> = self.rb.fetch_by_column("", "id", employee_id).await
            .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(result)
    }

    /// 更新员工信息
    pub async fn update_employee(&self, employee: &Employee) -> Result<()> {
        self.rb.update_by_column("", employee, "id").await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 列出所有员工
    pub async fn list_employees(&self) -> Result<Vec<Employee>> {
        let result: Vec<Employee> = self.rb.fetch_list("").await
            .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(result)
    }
}