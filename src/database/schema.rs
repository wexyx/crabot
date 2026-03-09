/// 数据库 schema 定义
/// 包括所有表的创建SQL语句
pub fn get_init_sql() -> String {
    r#"
    -- 公司表
    CREATE TABLE IF NOT EXISTS company (
        id VARCHAR(36) PRIMARY KEY,
        name VARCHAR(255) NOT NULL,
        ceo_id VARCHAR(36),
        total_employees INT DEFAULT 0,
        total_departments INT DEFAULT 0,
        founded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        description TEXT,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
    );

    -- 技能表
    CREATE TABLE IF NOT EXISTS skill (
        id VARCHAR(36) PRIMARY KEY,
        name VARCHAR(255) NOT NULL,
        description TEXT,
        level VARCHAR(50) NOT NULL,
        category VARCHAR(100) NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
    );

    -- 部门表
    CREATE TABLE IF NOT EXISTS department (
        id VARCHAR(36) PRIMARY KEY,
        name VARCHAR(255) NOT NULL,
        dept_type VARCHAR(50) NOT NULL,
        leader_id VARCHAR(36),
        parent_dept_id VARCHAR(36),
        employee_count INT DEFAULT 0,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        description TEXT,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
        FOREIGN KEY (parent_dept_id) REFERENCES department(id)
    );

    -- 员工表
    CREATE TABLE IF NOT EXISTS employee (
        id VARCHAR(36) PRIMARY KEY,
        name VARCHAR(255) NOT NULL,
        email VARCHAR(255) UNIQUE NOT NULL,
        role VARCHAR(50) NOT NULL,
        department_id VARCHAR(36),
        supervisor_id VARCHAR(36),
        hire_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        promotion_count INT DEFAULT 0,
        is_active BOOLEAN DEFAULT TRUE,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
        FOREIGN KEY (department_id) REFERENCES department(id),
        FOREIGN KEY (supervisor_id) REFERENCES employee(id)
    );

    -- 员工技能表（多对多关系）
    CREATE TABLE IF NOT EXISTS employee_skill (
        employee_id VARCHAR(36) NOT NULL,
        skill_id VARCHAR(36) NOT NULL,
        added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (employee_id, skill_id),
        FOREIGN KEY (employee_id) REFERENCES employee(id) ON DELETE CASCADE,
        FOREIGN KEY (skill_id) REFERENCES skill(id) ON DELETE CASCADE
    );

    -- 需求表
    CREATE TABLE IF NOT EXISTS requirement (
        id VARCHAR(36) PRIMARY KEY,
        description TEXT NOT NULL,
        client VARCHAR(255) NOT NULL,
        budget DECIMAL(15, 2) NOT NULL,
        status VARCHAR(50) NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
    );

    -- 销售成果表
    CREATE TABLE IF NOT EXISTS sale_result (
        id VARCHAR(36) PRIMARY KEY,
        requirement_id VARCHAR(36) NOT NULL,
        revenue DECIMAL(15, 2) NOT NULL,
        salesperson_id VARCHAR(36) NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (requirement_id) REFERENCES requirement(id),
        FOREIGN KEY (salesperson_id) REFERENCES employee(id)
    );

    -- 晋升记录表
    CREATE TABLE IF NOT EXISTS promotion_record (
        id VARCHAR(36) PRIMARY KEY,
        employee_id VARCHAR(36) NOT NULL,
        from_role VARCHAR(50) NOT NULL,
        to_role VARCHAR(50) NOT NULL,
        promoted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (employee_id) REFERENCES employee(id)
    );

    -- 索引优化查询性能
    CREATE INDEX idx_employee_department ON employee(department_id);
    CREATE INDEX idx_employee_supervisor ON employee(supervisor_id);
    CREATE INDEX idx_department_leader ON department(leader_id);
    CREATE INDEX idx_sale_result_salesperson ON sale_result(salesperson_id);
    CREATE INDEX idx_requirement_status ON requirement(status);
    "#.to_string()
}
