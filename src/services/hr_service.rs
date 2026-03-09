use crate::models::{Employee, Skill, SkillLevel, EmployeeRole};
use crate::{Result, CrabotError};
use uuid::Uuid;

/// HR部门服务
/// 负责：招聘新员工、员工信息管理、技能培训等
pub struct HRService;

impl HRService {
    /// 招聘新员工
    pub fn recruit_employee(
        name: String,
        email: String,
        department_id: Option<Uuid>,
        initial_skills: Vec<Skill>,
    ) -> Result<Employee> {
        // 验证邮箱格式
        if !email.contains('@') {
            return Err(CrabotError::InvalidOperation(
                "Invalid email format".to_string(),
            ));
        }

        let mut employee = Employee::new(name, email, department_id);

        // 添加初始技能
        for skill in initial_skills {
            employee.add_skill(skill);
        }

        Ok(employee)
    }

    /// 验证新员工技能是否满足部门要求
    pub fn validate_recruit_skills(
        employee: &Employee,
        required_skills: &[(String, SkillLevel)],
    ) -> Result<()> {
        if employee.has_required_skills(required_skills) {
            Ok(())
        } else {
            Err(CrabotError::SkillRequirementNotMet(
                "Employee skills do not meet department requirements".to_string(),
            ))
        }
    }

    /// 为员工安排培训（升级技能）
    pub fn provide_training(
        employee: &mut Employee,
        skill_name: &str,
        new_level: SkillLevel,
    ) -> Result<()> {
        if employee.upgrade_skill(skill_name, new_level) {
            Ok(())
        } else {
            Err(CrabotError::NotFound(format!(
                "Skill '{}' not found for employee",
                skill_name
            )))
        }
    }

    /// 解雇员工
    pub fn terminate_employee(employee: &mut Employee) -> Result<()> {
        if employee.role == EmployeeRole::CEO {
            return Err(CrabotError::InvalidOperation(
                "Cannot terminate CEO".to_string(),
            ));
        }
        employee.is_active = false;
        Ok(())
    }

    /// 生成招聘报告
    pub fn generate_recruitment_report(
        recruited_count: u32,
        total_employees: u32,
    ) -> String {
        format!(
            "招聘报告: 本月招聘 {} 人, 公司总员工数 {}",
            recruited_count, total_employees
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recruit_employee() {
        let skill = Skill::new(
            "沟通".to_string(),
            "communication".to_string(),
            SkillLevel::Intermediate,
            "soft-skills".to_string(),
        );

        let result = HRService::recruit_employee(
            "张三".to_string(),
            "zhangsan@bytedance.com".to_string(),
            None,
            vec![skill],
        );

        assert!(result.is_ok());
        let emp = result.unwrap();
        assert_eq!(emp.name, "张三");
        assert_eq!(emp.skills.len(), 1);
    }

    #[test]
    fn test_recruit_with_invalid_email() {
        let result = HRService::recruit_employee(
            "张三".to_string(),
            "invalid-email".to_string(),
            None,
            vec![],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_provide_training() {
        let skill = Skill::new(
            "销售".to_string(),
            "Sales".to_string(),
            SkillLevel::Beginner,
            "Sales".to_string(),
        );

        let mut emp = Employee::new(
            "李四".to_string(),
            "lisi@bytedance.com".to_string(),
            None,
        );
        emp.add_skill(skill);

        let result = HRService::provide_training(&mut emp, "销售", SkillLevel::Advanced);
        assert!(result.is_ok());

        let skill = emp.get_skill("销售").unwrap();
        assert_eq!(skill.level, SkillLevel::Advanced);
    }
}
