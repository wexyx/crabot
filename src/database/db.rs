use rbatis::rbatis::Rbatis;
use crate::{Result, CrabotError};
use crate::models::*;
use crate::services::sales_service::{Requirement, SaleResult};
use crate::middleware::database::DatabaseMiddleware;

use self::{
    company_db::CompanyDatabase,
    employee_db::EmployeeDatabase,
    skill_db::SkillDatabase,
    department_db::DepartmentDatabase,
    sales_db::SalesDatabase,
};

/// 数据库管理器
pub struct Database {
    middleware: DatabaseMiddleware,
}

impl Database {
    /// 初始化数据库连接池
    pub async fn new(database_url: &str) -> Result<Self> {
        let middleware = DatabaseMiddleware::new(database_url).await?;
        Ok(Self { middleware })
    }

    /// 初始化数据库表结构
    pub async fn init_schema(&self) -> Result<()> {
        self.middleware.init_schema().await
    }

    /// 获取Rbatis实例
    fn rb(&self) -> &Rbatis {
        self.middleware.rb()
    }

    // ========== 公司 相关操作 ==========

    /// 创建公司
    pub async fn create_company(&self, company: &Company) -> Result<()> {
        CompanyDatabase::new(self.rb()).create_company(company).await
    }

    /// 获取公司信息
    pub async fn get_company(&self, company_id: &str) -> Result<Option<Company>> {
        CompanyDatabase::new(self.rb()).get_company(company_id).await
    }

    // ========== 员工 相关操作 ==========

    /// 创建员工
    pub async fn create_employee(&self, employee: &Employee) -> Result<()> {
        EmployeeDatabase::new(self.rb()).create_employee(employee).await
    }

    /// 获取员工信息
    pub async fn get_employee(&self, employee_id: &str) -> Result<Option<Employee>> {
        EmployeeDatabase::new(self.rb()).get_employee(employee_id).await
    }

    /// 更新员工信息
    pub async fn update_employee(&self, employee: &Employee) -> Result<()> {
        EmployeeDatabase::new(self.rb()).update_employee(employee).await
    }

    /// 列出所有员工
    pub async fn list_employees(&self) -> Result<Vec<Employee>> {
        EmployeeDatabase::new(self.rb()).list_employees().await
    }

    // ========== 技能 相关操作 ==========

    /// 创建技能
    pub async fn create_skill(&self, skill: &Skill) -> Result<()> {
        SkillDatabase::new(self.rb()).create_skill(skill).await
    }

    /// 获取技能信息
    pub async fn get_skill(&self, skill_id: &str) -> Result<Option<Skill>> {
        SkillDatabase::new(self.rb()).get_skill(skill_id).await
    }

    // ========== 部门 相关操作 ==========

    /// 创建部门
    pub async fn create_department(&self, department: &Department) -> Result<()> {
        DepartmentDatabase::new(self.rb()).create_department(department).await
    }

    /// 获取部门信息
    pub async fn get_department(&self, department_id: &str) -> Result<Option<Department>> {
        DepartmentDatabase::new(self.rb()).get_department(department_id).await
    }

    /// 更新部门信息
    pub async fn update_department(&self, department: &Department) -> Result<()> {
        DepartmentDatabase::new(self.rb()).update_department(department).await
    }

    // ========== 需求和销售 ==========

    /// 创建需求
    pub async fn create_requirement(&self, requirement: &Requirement) -> Result<()> {
        SalesDatabase::new(self.rb()).create_requirement(requirement).await
    }

    /// 创建销售成果
    pub async fn create_sale_result(&self, sale: &SaleResult) -> Result<()> {
        SalesDatabase::new(self.rb()).create_sale_result(sale).await
    }

    /// 获取销售人员的业绩
    pub async fn get_salesperson_performance(&self, salesperson_id: &str) -> Result<f64> {
        SalesDatabase::new(self.rb()).get_salesperson_performance(salesperson_id).await
    }
}
