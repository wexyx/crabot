use crabot::{
    models::*,
    services::*,
    database::Database,
    Result,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("========== OpenClaw 组织管理系统演示 ==========\n");

    // 初始化数据库根据你的配置修改这个连接字符串
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:password@localhost/crabot".to_string());

    println!("📦 初始化数据库连接...");
    let db = match Database::new(&database_url).await {
        Ok(db) => {
            println!("✅ 数据库连接成功!");
            db
        }
        Err(e) => {
            eprintln!("❌ 数据库连接失败: {}", e);
            println!("\n请确保 MySQL 已启动，并运行以下命令创建数据库:");
            println!("  CREATE DATABASE IF NOT EXISTS crabot;");
            println!("\然后设置环境变量:");
            println!("  export DATABASE_URL=mysql://root:password@localhost/crabot");
            return Err(e);
        }
    };

    // 初始化数据库表
    println!("📊 初始化数据库表结构...");
    db.init_schema().await?;
    println!("✅ 表结构初始化完成!\n");

    // 1. 创建公司
    println!("=== 步骤 1: 创建公司 ===");
    let mut company = Company::new(
        "字节跳动".to_string(),
        "一家创意工程公司".to_string(),
    );
    db.create_company(&company).await?;
    println!("✅ 公司创建成功: {}\n", company.name);

    // 2. 创建部门
    println!("=== 步骤 2: 创建部门 ===");
    let mut hr_dept = Department::new(
        "人力资源部".to_string(),
        DepartmentType::HR,
        "负责招聘和员工管理".to_string(),
    );
    let mut sales_dept = Department::new(
        "销售部".to_string(),
        DepartmentType::Sales,
        "负责产品销售和客户沟通".to_string(),
    );
    let mut eng_dept = Department::new(
        "工程部".to_string(),
        DepartmentType::Engineering,
        "负责产品开发".to_string(),
    );

    db.create_department(&hr_dept).await?;
    db.create_department(&sales_dept).await?;
    db.create_department(&eng_dept).await?;

    println!("✅ 部门创建成功:");
    println!("   - {}", hr_dept.name);
    println!("   - {}", sales_dept.name);
    println!("   - {}\n", eng_dept.name);

    // 3. 创建技能
    println!("=== 步骤 3: 创建技能体系 ===");
    let skills = vec![
        Skill::new(
            "招聘".to_string(),
            "人才招聘和筛选能力".to_string(),
            SkillLevel::Intermediate,
            "HR".to_string(),
        ),
        Skill::new(
            "员工管理".to_string(),
            "员工管理和发展能力".to_string(),
            SkillLevel::Intermediate,
            "HR".to_string(),
        ),
        Skill::new(
            "销售".to_string(),
            "产品销售和客户维护能力".to_string(),
            SkillLevel::Intermediate,
            "Sales".to_string(),
        ),
        Skill::new(
            "沟通".to_string(),
            "团队沟通能力".to_string(),
            SkillLevel::Advanced,
            "SoftSkills".to_string(),
        ),
        Skill::new(
            "管理".to_string(),
            "团队管理能力".to_string(),
            SkillLevel::Intermediate,
            "Management".to_string(),
        ),
        Skill::new(
            "编程".to_string(),
            "代码编写能力".to_string(),
            SkillLevel::Advanced,
            "Technical".to_string(),
        ),
    ];

    for skill in &skills {
        db.create_skill(skill).await?;
    }
    println!("✅ 创建了 {} 个技能\n", skills.len());

    // 4. HR部门招聘新员工
    println!("=== 步骤 4: HR 部门招聘员工 ===");
    let recruit_skills = vec![skills[0].clone(), skills[1].clone()];
    let hr_employee = HRService::recruit_employee(
        "王芳".to_string(),
        "wangfang@bytedance.com".to_string(),
        Some(hr_dept.id),
        recruit_skills,
    )?;

    let sales_employee = HRService::recruit_employee(
        "李时珍".to_string(),
        "lishizhen@bytedance.com".to_string(),
        Some(sales_dept.id),
        vec![skills[2].clone(), skills[3].clone()],
    )?;

    let eng_employee = HRService::recruit_employee(
        "钱学森".to_string(),
        "qianxuesen@bytedance.com".to_string(),
        Some(eng_dept.id),
        vec![skills[5].clone()],
    )?;

    db.create_employee(&hr_employee).await?;
    db.create_employee(&sales_employee).await?;
    db.create_employee(&eng_employee).await?;

    company.add_employee();
    company.add_employee();
    company.add_employee();

    println!("✅ 招聘了 3 名员工:");
    println!("   - {} (HR部门)", hr_employee.name);
    println!("   - {} (销售部门)", sales_employee.name);
    println!("   - {} (工程部门)\n", eng_employee.name);

    // 5. 销售部门处理需求
    println!("=== 步骤 5: 销售部门收集和处理需求 ===");
    let mut req1 = SalesService::collect_requirement(
        "需要一个在线销售系统".to_string(),
        "阿里巴巴".to_string(),
        100000.0,
    )?;

    SalesService::process_requirement(&mut req1, &sales_employee)?;
    db.create_requirement(&req1).await?;

    let sale_result = SalesService::complete_sale(req1.id, sales_employee.id, 80000.0)?;
    db.create_sale_result(&sale_result).await?;

    let performance = db.get_salesperson_performance(&sale_result.salesperson_id.to_string()).await?;
    println!("✅ 销售成果:");
    println!("   - 需求: {}", req1.description);
    println!("   - 客户: {}", req1.client);
    println!("   - 实际收入: ¥{:.2}", sale_result.revenue);
    println!("   - 销售员工: {} 的总业绩: ¥{:.2}\n", sales_employee.name, performance);

    // 6. 员工晋升
    println!("=== 步骤 6: 员工晋升流程 ===");
    let mut promotable_emp = Employee::new(
        "张珂".to_string(),
        "zhangke@bytedance.com".to_string(),
        Some(sales_dept.id),
    );

    // 添加晋升所需的技能
    promotable_emp.add_skill(skills[3].clone()); // 沟通
    promotable_emp.add_skill(skills[4].clone()); // 管理

    db.create_employee(&promotable_emp).await?;

    match PromotionService::promote_to_leader(&mut promotable_emp, &mut sales_dept) {
        Ok(_) => {
            println!("✅ 晋升成功!");
            println!("   - 员工: {}", promotable_emp.name);
            println!("   - 新职位: {}", promotable_emp.role.to_string());
            db.update_employee(&promotable_emp).await?;
            db.update_department(&sales_dept).await?;
        }
        Err(e) => println!("❌ 晋升失败: {}", e),
    }

    println!("\n=== 系统初始化完成 ===");
    println!("数据已保存到 MySQL 数据库，支持系统重启恢复\n");

    Ok(())
}
