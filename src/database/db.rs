use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;  // 导入 Row trait
use crate::{Result, CrabotError};
use crate::models::*;
use crate::database::schema;
use uuid::Uuid;
use std::time::Duration;

/// 数据库管理器
pub struct Database {
    pool: MySqlPool,
}

impl Database {
    /// 初始化数据库连接池
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await
            .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// 初始化数据库表结构
    pub async fn init_schema(&self) -> Result<()> {
        let sql = schema::get_init_sql();
        // 分割SQL语句并执行
        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("--") {
                sqlx::query(trimmed)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| CrabotError::DatabaseError(format!("Failed to execute schema: {}", e)))?;
            }
        }
        Ok(())
    }

    // ========== 公司 相关操作 ==========
    
    /// 创建公司
    pub async fn create_company(&self, company: &Company) -> Result<()> {
        sqlx::query(
            "INSERT INTO company (id, name, ceo_id, total_employees, total_departments, founded_at, description) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(company.id.to_string())
        .bind(&company.name)
        .bind(company.ceo_id.map(|id| id.to_string()))
        .bind(company.total_employees)
        .bind(company.total_departments)
        .bind(company.founded_at)
        .bind(&company.description)
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取公司信息
    pub async fn get_company(&self, company_id: &str) -> Result<Option<Company>> {
        let row = sqlx::query(
            "SELECT id, name, ceo_id, total_employees, total_departments, founded_at, description 
             FROM company WHERE id = ?"
        )
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| {
            let id: String = r.get("id");
            let ceo_id: Option<String> = r.get("ceo_id");
            Company {
                id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                name: r.get("name"),
                ceo_id: ceo_id.and_then(|id| Uuid::parse_str(&id).ok()),
                total_employees: r.get("total_employees"),
                total_departments: r.get("total_departments"),
                founded_at: r.get("founded_at"),
                description: r.get("description"),
            }
        }))
    }

    // ========== 员工 相关操作 ==========

    /// 创建员工
    pub async fn create_employee(&self, employee: &Employee) -> Result<()> {
        sqlx::query(
            "INSERT INTO employee (id, name, email, role, department_id, supervisor_id, hire_date, promotion_count, is_active) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(employee.id.to_string())
        .bind(&employee.name)
        .bind(&employee.email)
        .bind(employee.role.to_string())
        .bind(employee.department_id.map(|id| id.to_string()))
        .bind(employee.supervisor_id.map(|id| id.to_string()))
        .bind(employee.hire_date)
        .bind(employee.promotion_count)
        .bind(employee.is_active)
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        // 添加员工的技能
        for skill in &employee.skills {
            self.add_employee_skill(&employee.id, &skill.id).await?;
        }

        Ok(())
    }

    /// 获取员工信息
    pub async fn get_employee(&self, employee_id: &str) -> Result<Option<Employee>> {
        let row = sqlx::query(
            "SELECT id, name, email, role, department_id, supervisor_id, hire_date, promotion_count, is_active 
             FROM employee WHERE id = ?"
        )
        .bind(employee_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        if let Some(r) = row {
            let emp_id: String = r.get("id");
            let dept_id: Option<String> = r.get("department_id");
            let super_id: Option<String> = r.get("supervisor_id");

            let mut employee = Employee {
                id: Uuid::parse_str(&emp_id).unwrap_or_else(|_| Uuid::new_v4()),
                name: r.get("name"),
                email: r.get("email"),
                role: r.get::<String, _>("role").into(),
                department_id: dept_id.and_then(|id| Uuid::parse_str(&id).ok()),
                supervisor_id: super_id.and_then(|id| Uuid::parse_str(&id).ok()),
                hire_date: r.get("hire_date"),
                promotion_count: r.get("promotion_count"),
                is_active: r.get("is_active"),
                skills: Vec::new(),
            };

            // 获取员工的技能
            employee.skills = self.get_employee_skills(&employee.id).await?;

            Ok(Some(employee))
        } else {
            Ok(None)
        }
    }

    /// 更新员工信息
    pub async fn update_employee(&self, employee: &Employee) -> Result<()> {
        sqlx::query(
            "UPDATE employee SET name = ?, email = ?, role = ?, department_id = ?, supervisor_id = ?, promotion_count = ?, is_active = ? WHERE id = ?"
        )
        .bind(&employee.name)
        .bind(&employee.email)
        .bind(employee.role.to_string())
        .bind(employee.department_id.map(|id| id.to_string()))
        .bind(employee.supervisor_id.map(|id| id.to_string()))
        .bind(employee.promotion_count)
        .bind(employee.is_active)
        .bind(employee.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 列出所有员工
    pub async fn list_employees(&self) -> Result<Vec<Employee>> {
        let rows = sqlx::query(
            "SELECT id, name, email, role, department_id, supervisor_id, hire_date, promotion_count, is_active 
             FROM employee"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        let mut employees = Vec::new();
        for r in rows {
            let emp_id: String = r.get("id");
            let dept_id: Option<String> = r.get("department_id");
            let super_id: Option<String> = r.get("supervisor_id");

            let emp_uuid = Uuid::parse_str(&emp_id).unwrap_or_else(|_| Uuid::new_v4());
            let skills = self.get_employee_skills(&emp_uuid).await.unwrap_or_default();

            let employee = Employee {
                id: emp_uuid,
                name: r.get("name"),
                email: r.get("email"),
                role: r.get::<String, _>("role").into(),
                department_id: dept_id.and_then(|id| Uuid::parse_str(&id).ok()),
                supervisor_id: super_id.and_then(|id| Uuid::parse_str(&id).ok()),
                hire_date: r.get("hire_date"),
                promotion_count: r.get("promotion_count"),
                is_active: r.get("is_active"),
                skills,
            };

            employees.push(employee);
        }

        Ok(employees)
    }

    // ========== 技能 相关操作 ==========

    /// 创建技能
    pub async fn create_skill(&self, skill: &Skill) -> Result<()> {
        sqlx::query(
            "INSERT INTO skill (id, name, description, level, category) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(skill.id.to_string())
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(skill.level.to_string())
        .bind(&skill.category)
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取技能信息
    pub async fn get_skill(&self, skill_id: &str) -> Result<Option<Skill>> {
        let row = sqlx::query(
            "SELECT id, name, description, level, category FROM skill WHERE id = ?"
        )
        .bind(skill_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| {
            let id: String = r.get("id");
            Skill {
                id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                name: r.get("name"),
                description: r.get("description"),
                level: r.get::<String, _>("level").into(),
                category: r.get("category"),
            }
        }))
    }

    // ========== 部门 相关操作 ==========

    /// 创建部门
    pub async fn create_department(&self, department: &Department) -> Result<()> {
        sqlx::query(
            "INSERT INTO department (id, name, dept_type, leader_id, parent_dept_id, employee_count, description) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(department.id.to_string())
        .bind(&department.name)
        .bind(department.dept_type.to_string())
        .bind(department.leader_id.map(|id| id.to_string()))
        .bind(department.parent_dept_id.map(|id| id.to_string()))
        .bind(department.employee_count)
        .bind(&department.description)
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取部门信息
    pub async fn get_department(&self, department_id: &str) -> Result<Option<Department>> {
        let row = sqlx::query(
            "SELECT id, name, dept_type, leader_id, parent_dept_id, employee_count, created_at, description 
             FROM department WHERE id = ?"
        )
        .bind(department_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| {
            let id: String = r.get("id");
            let leader_id: Option<String> = r.get("leader_id");
            let parent_id: Option<String> = r.get("parent_dept_id");
            Department {
                id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                name: r.get("name"),
                dept_type: r.get::<String, _>("dept_type").into(),
                leader_id: leader_id.and_then(|id| Uuid::parse_str(&id).ok()),
                parent_dept_id: parent_id.and_then(|id| Uuid::parse_str(&id).ok()),
                employee_count: r.get("employee_count"),
                created_at: r.get("created_at"),
                description: r.get("description"),
            }
        }))
    }

    /// 更新部门信息
    pub async fn update_department(&self, department: &Department) -> Result<()> {
        sqlx::query(
            "UPDATE department SET name = ?, dept_type = ?, leader_id = ?, parent_dept_id = ?, employee_count = ?, description = ? WHERE id = ?"
        )
        .bind(&department.name)
        .bind(department.dept_type.to_string())
        .bind(department.leader_id.map(|id| id.to_string()))
        .bind(department.parent_dept_id.map(|id| id.to_string()))
        .bind(department.employee_count)
        .bind(&department.description)
        .bind(department.id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    // ========== 员工技能关联 ==========

    /// 添加员工技能
    async fn add_employee_skill(&self, employee_id: &Uuid, skill_id: &Uuid) -> Result<()> {
        sqlx::query(
            "INSERT IGNORE INTO employee_skill (employee_id, skill_id) VALUES (?, ?)"
        )
        .bind(employee_id.to_string())
        .bind(skill_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取员工的技能列表
    async fn get_employee_skills(&self, employee_id: &Uuid) -> Result<Vec<Skill>> {
        let rows = sqlx::query(
            "SELECT s.id, s.name, s.description, s.level, s.category 
             FROM skill s
             INNER JOIN employee_skill es ON s.id = es.skill_id
             WHERE es.employee_id = ?"
        )
        .bind(employee_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        let skills = rows.iter().map(|r| {
            let id: String = r.get("id");
            Skill {
                id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                name: r.get("name"),
                description: r.get("description"),
                level: r.get::<String, _>("level").into(),
                category: r.get("category"),
            }
        }).collect();

        Ok(skills)
    }

    // ========== 需求和销售 ==========

    /// 创建需求
    pub async fn create_requirement(&self, requirement: &crate::services::sales_service::Requirement) -> Result<()> {
        sqlx::query(
            "INSERT INTO requirement (id, description, client, budget, status) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(requirement.id.to_string())
        .bind(&requirement.description)
        .bind(&requirement.client)
        .bind(requirement.budget)
        .bind(format!("{:?}", requirement.status).to_lowercase())
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 创建销售成果
    pub async fn create_sale_result(&self, sale: &crate::services::sales_service::SaleResult) -> Result<()> {
        sqlx::query(
            "INSERT INTO sale_result (id, requirement_id, revenue, salesperson_id) VALUES (?, ?, ?, ?)"
        )
        .bind(sale.id.to_string())
        .bind(sale.requirement_id.to_string())
        .bind(sale.revenue)
        .bind(sale.salesperson_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// 获取销售人员的业绩
    pub async fn get_salesperson_performance(&self, salesperson_id: &str) -> Result<f64> {
        let row = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT SUM(revenue) FROM sale_result WHERE salesperson_id = ?"
        )
        .bind(salesperson_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CrabotError::DatabaseError(e.to_string()))?;

        Ok(row.flatten().unwrap_or(0.0))
    }
}
