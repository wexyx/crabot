use rbatis::rbatis::Rbatis;
use crate::{Result, CrabotError};
use crate::models::Company;

/// 公司数据库操作
pub struct CompanyDatabase<'a> {
    rb: &'a Rbatis,
}

impl<'a> CompanyDatabase<'a> {
    pub fn new(rb: &'a Rbatis) -> Self {
        Self { rb }
    }

    /// 创建公司
    pub async fn create_company(&self, company: &Company) -> Result<()> {
        self.rb.save("", company).await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取公司信息
    pub async fn get_company(&self, company_id: &str) -> Result<Option<Company>> {
        let result: Option<Company> = self.rb.fetch_by_column("", "id", company_id).await
            .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(result)
    }
}