use crate::models::{Employee, EmployeeRole, Department, SkillLevel};
use crate::{Result, CrabotError};

/// 晋升服务
/// 负责：员工晋升管理、晋升条件评估
pub struct PromotionService;

impl PromotionService {
    /// 评估员工是否可以晋升为部门leader
    pub fn can_promote_to_leader(
        employee: &Employee,
        department: &Department,
    ) -> Result<()> {
        // 检查职位
        if employee.role != EmployeeRole::Staff {
            return Err(CrabotError::PromotionFailed(
                "Only staff can be promoted to leader".to_string(),
            ));
        }

        // 检查是否在该部门
        if employee.department_id != Some(department.id) {
            return Err(CrabotError::PromotionFailed(
                "Employee must be in this department".to_string(),
            ));
        }

        // 检查工作经验（晋升次数作为代理）
        if employee.promotion_count > 0 {
            return Err(CrabotError::PromotionFailed(
                "Employee already has been promoted".to_string(),
            ));
        }

        // 检查技能要求
        let required_skills = vec![
            ("管理".to_string(), SkillLevel::Intermediate),
            ("沟通".to_string(), SkillLevel::Advanced),
        ];

        if !employee.has_required_skills(&required_skills) {
            return Err(CrabotError::SkillRequirementNotMet(
                "Employee does not have required skills for leader position".to_string(),
            ));
        }

        Ok(())
    }

    /// 评估leader是否可以晋升为CEO
    pub fn can_promote_to_ceo(employee: &Employee) -> Result<()> {
        // 检查职位
        if employee.role != EmployeeRole::Leader {
            return Err(CrabotError::PromotionFailed(
                "Only leaders can be promoted to CEO".to_string(),
            ));
        }

        // 检查最少晋升次数（作为经验要求）
        if employee.promotion_count < 1 {
            return Err(CrabotError::PromotionFailed(
                "Leader must have sufficient experience".to_string(),
            ));
        }

        // 检查CEO必需的技能
        let required_skills = vec![
            ("战略规划".to_string(), SkillLevel::Advanced),
            ("决策".to_string(), SkillLevel::Expert),
        ];

        if !employee.has_required_skills(&required_skills) {
            return Err(CrabotError::SkillRequirementNotMet(
                "Employee does not have required skills for CEO position".to_string(),
            ));
        }

        Ok(())
    }

    /// 执行员工晋升为leader
    pub fn promote_to_leader(
        employee: &mut Employee,
        department: &mut Department,
    ) -> Result<()> {
        Self::can_promote_to_leader(employee, department)?;

        employee.promote_to_leader();
        department.set_leader(employee.id);

        Ok(())
    }

    /// 执行leader晋升为CEO
    pub fn promote_to_ceo(employee: &mut Employee) -> Result<()> {
        Self::can_promote_to_ceo(employee)?;

        employee.promote_to_ceo();

        Ok(())
    }

    /// 生成晋升报告
    pub fn generate_promotion_report(
        employee: &Employee,
        promotion_type: &str,
    ) -> String {
        format!(
            "晋升报告:\n员工: {}\n晋升类型: {}\n新职位: {}\n总晋升次数: {}",
            employee.name,
            promotion_type,
            employee.role.to_string(),
            employee.promotion_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Skill, DepartmentType};

    #[test]
    fn test_promote_to_leader() {
        let dept_id = uuid::Uuid::new_v4();
        let mut dept = Department::new(
            "销售部".to_string(),
            DepartmentType::Sales,
            "销售部门".to_string(),
        );
        dept.id = dept_id;

        let mut emp = Employee::new(
            "张三".to_string(),
            "zhangsan@bytedance.com".to_string(),
            Some(dept_id),
        );

        // 添加必需的技能
        emp.add_skill(Skill::new(
            "管理".to_string(),
            "管理技能".to_string(),
            SkillLevel::Intermediate,
            "management".to_string(),
        ));
        emp.add_skill(Skill::new(
            "沟通".to_string(),
            "沟通技能".to_string(),
            SkillLevel::Advanced,
            "soft-skills".to_string(),
        ));

        // 执行晋升
        let result = PromotionService::promote_to_leader(&mut emp, &mut dept);
        assert!(result.is_ok());
        assert_eq!(emp.role, EmployeeRole::Leader);
        assert_eq!(dept.leader_id, Some(emp.id));
    }

    #[test]
    fn test_promote_to_ceo() {
        let mut emp = Employee::new(
            "李四".to_string(),
            "lisi@bytedance.com".to_string(),
            None,
        );

        // 设置为leader并添加经验
        emp.role = EmployeeRole::Leader;
        emp.promotion_count = 1;

        // 添加CEO必需的技能
        emp.add_skill(Skill::new(
            "战略规划".to_string(),
            "战略规划".to_string(),
            SkillLevel::Advanced,
            "strategy".to_string(),
        ));
        emp.add_skill(Skill::new(
            "决策".to_string(),
            "决策技能".to_string(),
            SkillLevel::Expert,
            "decision-making".to_string(),
        ));

        let result = PromotionService::promote_to_ceo(&mut emp);
        assert!(result.is_ok());
        assert_eq!(emp.role, EmployeeRole::CEO);
    }
}
