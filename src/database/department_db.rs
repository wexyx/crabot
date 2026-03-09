use rbatis::rbatis::Rbatis;
use crate::{Result, CrabotError};
use crate::models::Department;

/// 部门数据库操作
pub struct DepartmentDatabase<'a> {
    rb: &'a Rbatis,
}

impl<'a> DepartmentDatabase<'a> {
    pub fn new(rb: &'a Rbatis) -> Self {
        Self { rb }
    }

    /// 创建部门
    pub async fn create_department(&self, department: &Department) -> Result<()> {
        self.rb.save("", department).await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取部门信息
    pub async fn get_department(&self, department_id: &str) -> Result<Option<Department>> {
        let result: Option<Department> = self.rb.fetch_by_column("", "id", department_id).await
            .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(result)
    }

    /// 更新部门信息
    pub async fn update_department(&self, department: &Department) -> Result<()> {
        self.rb.update_by_column("", department, "id").await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}