use crate::models::{Employee, EmployeeRole};
use crate::{Result, CrabotError};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

/// 需求信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub id: Uuid,
    pub description: String,
    pub client: String,
    pub budget: f64,
    pub status: RequirementStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequirementStatus {
    Collected,
    InProgress,
    Completed,
    Archived,
}

/// 销售成果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleResult {
    pub id: Uuid,
    pub requirement_id: Uuid,
    pub revenue: f64,
    pub salesperson_id: Uuid,
}

/// 销售部门服务
/// 负责：需求收集、客户沟通、合同达成、收入统计
pub struct SalesService;

impl SalesService {
    /// 收集外部需求（通过客户沟通）
    pub fn collect_requirement(
        description: String,
        client: String,
        budget: f64,
    ) -> Result<Requirement> {
        if budget <= 0.0 {
            return Err(CrabotError::InvalidOperation(
                "Budget must be positive".to_string(),
            ));
        }

        Ok(Requirement {
            id: Uuid::new_v4(),
            description,
            client,
            budget,
            status: RequirementStatus::Collected,
        })
    }

    /// 处理需求（销售员工负责沟通并推进）
    pub fn process_requirement(
        requirement: &mut Requirement,
        salesperson: &Employee,
    ) -> Result<()> {
        // 验证销售员工是否有足够的销售技能
        if !salesperson
            .get_skill("销售")
            .map(|s| s.meets_requirement(crate::models::SkillLevel::Beginner))
            .unwrap_or(false)
        {
            return Err(CrabotError::SkillRequirementNotMet(
                "Salesperson does not have required sales skills".to_string(),
            ));
        }

        requirement.status = RequirementStatus::InProgress;
        Ok(())
    }

    /// 完成销售（生成销售成果）
    pub fn complete_sale(
        requirement_id: Uuid,
        salesperson_id: Uuid,
        actual_revenue: f64,
    ) -> Result<SaleResult> {
        if actual_revenue <= 0.0 {
            return Err(CrabotError::InvalidOperation(
                "Revenue must be positive".to_string(),
            ));
        }

        Ok(SaleResult {
            id: Uuid::new_v4(),
            requirement_id,
            revenue: actual_revenue,
            salesperson_id,
        })
    }

    /// 生成销售报告
    pub fn generate_sales_report(
        total_revenue: f64,
        total_requirements: u32,
        completed_requirements: u32,
    ) -> String {
        let completion_rate = if total_requirements > 0 {
            (completed_requirements as f64 / total_requirements as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "销售报告:\n总收入: ¥{:.2}\n总需求数: {}\n完成需求数: {}\n完成率: {:.1}%",
            total_revenue, total_requirements, completed_requirements, completion_rate
        )
    }

    /// 计算销售业绩（基于销售成果）
    pub fn calculate_performance(
        salesperson: &Employee,
        sales_results: &[SaleResult],
    ) -> f64 {
        sales_results
            .iter()
            .filter(|result| result.salesperson_id == salesperson.id)
            .map(|result| result.revenue)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_requirement() {
        let result = SalesService::collect_requirement(
            "需要一个在线销售系统".to_string(),
            "阿里巴巴".to_string(),
            100000.0,
        );

        assert!(result.is_ok());
        let req = result.unwrap();
        assert_eq!(req.client, "阿里巴巴");
        assert_eq!(req.status, RequirementStatus::Collected);
    }

    #[test]
    fn test_collect_requirement_with_invalid_budget() {
        let result = SalesService::collect_requirement(
            "需要一个系统".to_string(),
            "客户".to_string(),
            -1000.0,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_complete_sale() {
        let req_id = Uuid::new_v4();
        let salesperson_id = Uuid::new_v4();

        let result = SalesService::complete_sale(req_id, salesperson_id, 50000.0);

        assert!(result.is_ok());
        let sale = result.unwrap();
        assert_eq!(sale.revenue, 50000.0);
    }
}
