use rbatis::rbatis::Rbatis;
use crate::{Result, CrabotError};
use crate::services::sales_service::{Requirement, SaleResult};

/// 销售数据库操作
pub struct SalesDatabase<'a> {
    rb: &'a Rbatis,
}

impl<'a> SalesDatabase<'a> {
    pub fn new(rb: &'a Rbatis) -> Self {
        Self { rb }
    }

    /// 创建需求
    pub async fn create_requirement(&self, requirement: &Requirement) -> Result<()> {
        let sql = "INSERT INTO requirement (id, description, client, budget, status) VALUES (?, ?, ?, ?, ?)";
        self.rb.exec(sql, vec![
            rbatis::value!(requirement.id.to_string()),
            rbatis::value!(requirement.description.clone()),
            rbatis::value!(requirement.client.clone()),
            rbatis::value!(requirement.budget),
            rbatis::value!(format!("{:?}", requirement.status).to_lowercase()),
        ]).await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 创建销售成果
    pub async fn create_sale_result(&self, sale: &SaleResult) -> Result<()> {
        let sql = "INSERT INTO sale_result (id, requirement_id, revenue, salesperson_id) VALUES (?, ?, ?, ?)";
        self.rb.exec(sql, vec![
            rbatis::value!(sale.id.to_string()),
            rbatis::value!(sale.requirement_id.to_string()),
            rbatis::value!(sale.revenue),
            rbatis::value!(sale.salesperson_id.to_string()),
        ]).await.map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取销售人员的业绩
    pub async fn get_salesperson_performance(&self, salesperson_id: &str) -> Result<f64> {
        let sql = "SELECT SUM(revenue) FROM sale_result WHERE salesperson_id = ?";
        let result: Option<f64> = self.rb.fetch(sql, vec![rbatis::value!(salesperson_id)]).await
            .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(result.unwrap_or(0.0))
    }
}