# OpenClaw - 组织管理系统

一个基于字节跳动公司组织架构设计理念的开源组织管理系统，使用 Rust 和 MySQL 实现。这个系统模拟了一个类似字节跳动的企业组织结构，包括公司、部门、员工、技能体系等。

## 系统功能

### 核心功能
- **公司管理**: 创建和管理公司、CEO任职、员工统计
- **部门管理**: 创建各种部门（HR、销售、工程、运营、市场），管理部门结构
- **员工管理**: 招聘、员工信息维护、员工角色转变
- **技能体系**: 定义技能等级、技能分类、员工技能追踪
- **晋升机制**: 员工→部门Leader→CEO的晋升流程，带有技能要求验证
- **销售管理**: 需求收集、销售处理、收入跟踪、业绩统计
- **HR服务**: 新员工招聘、技能培训、员工解雇

### 部门功能

#### HR部门
- 招聘新员工（带有初始技能配置）
- 为员工提供培训（升级技能等级）
- 解雇员工
- 生成招聘报告

#### 销售部门
- 通过客户沟通收集需求
- 处理和跟踪需求
- 完成销售交易
- 统计销售业绩

#### 其他部门
- 工程部、运营部、市场部等部门的员工管理

### 数据持久化
- 所有数据保存到 MySQL 数据库
- 支持系统重启后的数据恢复
- 完整的表结构和索引优化

## 项目架构

```
src/
├── main.rs              # 应用入口，演示系统功能
├── lib.rs               # 库根模块
├── error.rs             # 错误处理定义
├── models/              # 数据模型
│   ├── mod.rs
│   ├── skill.rs         # 技能模型
│   ├── employee.rs      # 员工模型
│   ├── department.rs    # 部门模型
│   └── company.rs       # 公司模型
├── services/            # 业务逻辑服务
│   ├── mod.rs
│   ├── hr_service.rs    # HR服务
│   ├── sales_service.rs # 销售服务
│   └── promotion_service.rs # 晋升服务
└── database/            # 数据库访问层
    ├── mod.rs
    ├── db.rs            # 数据库操作实现
    └── schema.rs        # 数据库表定义
```

## 系统要求

- Rust 1.70+ (推荐 1.75+)
- MySQL 5.7+ 或 MySQL 8.0+
- Tokio 异步运行时

## 快速开始

### 1. 数据库准备

首先创建数据库：

```bash
# 连接到 MySQL
mysql -u root -p

# 创建数据库
CREATE DATABASE IF NOT EXISTS crabot;
CREATE USER IF NOT EXISTS 'crabot'@'localhost' IDENTIFIED BY 'crabot_password';
GRANT ALL PRIVILEGES ON crabot.* TO 'crabot'@'localhost';
FLUSH PRIVILEGES;
EXIT;
```

### 2. 配置环境变量

```bash
export DATABASE_URL=mysql://crabot:crabot_password@localhost/crabot
```

### 3. 构建项目

```bash
cargo build --release
```

### 4. 运行应用

```bash
cargo run --release
```

应用启动时会自动初始化数据库表结构。

## 核心数据模型

### 技能 (Skill)
```rust
pub struct Skill {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub level: SkillLevel,    // Beginner, Intermediate, Advanced, Expert
    pub category: String,      // Sales, Management, Technical, SoftSkills
}
```

### 员工 (Employee)
```rust
pub struct Employee {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: EmployeeRole,    // Staff, Leader, CEO
    pub department_id: Option<Uuid>,
    pub supervisor_id: Option<Uuid>,
    pub skills: Vec<Skill>,
    pub hire_date: DateTime<Utc>,
    pub promotion_count: u32,
    pub is_active: bool,
}
```

### 部门 (Department)
```rust
pub struct Department {
    pub id: Uuid,
    pub name: String,
    pub dept_type: DepartmentType,    // HR, Sales, Engineering, Operations, Marketing
    pub leader_id: Option<Uuid>,
    pub parent_dept_id: Option<Uuid>,
    pub employee_count: u32,
    pub created_at: DateTime<Utc>,
    pub description: String,
}
```

### 公司 (Company)
```rust
pub struct Company {
    pub id: Uuid,
    pub name: String,
    pub ceo_id: Option<Uuid>,
    pub total_employees: u32,
    pub total_departments: u32,
    pub founded_at: DateTime<Utc>,
    pub description: String,
}
```

## 主要服务

### HRService - 人力资源服务
```rust
// 招聘新员工
let employee = HRService::recruit_employee(
    "张三".to_string(),
    "zhangsan@bytedance.com".to_string(),
    Some(dept_id),
    vec![skill1, skill2],
)?;

// 提供培训（升级技能）
HRService::provide_training(&mut employee, "销售", SkillLevel::Advanced)?;

// 解雇员工
HRService::terminate_employee(&mut employee)?;
```

### SalesService - 销售服务
```rust
// 收集需求
let requirement = SalesService::collect_requirement(
    "需要一个在线系统".to_string(),
    "客户名称".to_string(),
    100000.0,
)?;

// 处理需求
SalesService::process_requirement(&mut requirement, &salesperson)?;

// 完成销售
let sale = SalesService::complete_sale(req_id, salesperson_id, 80000.0)?;

// 生成销售报告
let report = SalesService::generate_sales_report(total_revenue, total_reqs, completed_reqs);
```

### PromotionService - 晋升服务
```rust
// 评估和执行晋升为Leader
PromotionService::promote_to_leader(&mut employee, &mut department)?;

// 评估和执行晋升为CEO
PromotionService::promote_to_ceo(&mut employee)?;

// 生成晋升报告
let report = PromotionService::generate_promotion_report(&employee, "晋升类型");
```

## 数据库表结构

系统自动创建以下表：

- `company` - 公司信息
- `employee` - 员工信息
- `department` - 部门信息
- `skill` - 技能定义
- `employee_skill` - 员工与技能的多对多关系
- `requirement` - 销售需求
- `sale_result` - 销售成果
- `promotion_record` - 晋升记录

## 示例流程

1. **创建公司和部门**
   ```rust
   let company = Company::new("字节跳动".to_string(), "描述".to_string());
   let hr_dept = Department::new("HR部门".to_string(), DepartmentType::HR, "描述".to_string());
   ```

2. **招聘员工**
   ```rust
   let employee = HRService::recruit_employee(
       "张三".to_string(),
       "zhangsan@bytedance.com".to_string(),
       Some(hr_dept.id),
       vec![招聘技能, 员工管理技能],
   )?;
   ```

3. **处理销售需求**
   ```rust
   let mut req = SalesService::collect_requirement(...)?;
   SalesService::process_requirement(&mut req, &salesperson)?;
   let sale = SalesService::complete_sale(...)?;
   ```

4. **员工晋升**
   ```rust
   PromotionService::promote_to_leader(&mut employee, &mut department)?;
   PromotionService::promote_to_ceo(&mut employee)?;
   ```

5. **数据持久化**
   ```rust
   let db = Database::new(&database_url).await?;
   db.init_schema().await?;
   db.create_employee(&employee).await?;
   db.create_requirement(&requirement).await?;
   ```

## 技能要求

### 部门Leader晋升条件
- 职位：必须是Staff
- 技能：管理(Intermediate)、沟通(Advanced)
- 经验：首次晋升

### CEO晋升条件
- 职位：必须是Leader
- 技能：战略规划(Advanced)、决策(Expert)
- 经验：至少1次晋升

## 测试

项目包含单元测试，可以运行：

```bash
cargo test

# 运行特定测试
cargo test test_promote_to_leader

# 显示测试输出
cargo test -- --nocapture
```

## 错误处理

系统定义了完整的错误类型：

```rust
pub enum CrabotError {
    DatabaseError(String),
    NotFound(String),
    InvalidOperation(String),
    PermissionDenied(String),
    SkillRequirementNotMet(String),
    PromotionFailed(String),
    SqlError(sqlx::Error),
    Unknown(String),
}
```

## 特性

- ✅ 完整的组织架构管理
- ✅ 灵活的技能体系和追踪
- ✅ 自动化的晋升流程和验证
- ✅ MySQL数据持久化
- ✅ 异步操作支持(Tokio)
- ✅ 完善的错误处理
- ✅ 单元测试覆盖

## 扩展计划

- [ ] Web API (REST/GraphQL)
- [ ] 员工晋升历史记录
- [ ] 部门绩效评分
- [ ] 员工绩效评分系统
- [ ] 工作流审批系统
- [ ] 权限管理系统
- [ ] 前端管理系统

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

## 联系方式

如有问题或建议，请通过 GitHub Issues 联系我们。


## 概述

`crabot` 是一个基于公司组织架构的多智能体（AI Agent）系统。  
系统以公司、部门、员工、顾问为核心结构，实现任务分配、执行与绩效管理。

- **公司（Company）**：承载系统整体，管理部门和资源。
- **部门（Department）**：按职能划分，管理员工和内部任务。
- **员工（Employee / Skill）**：执行具体技能，如搜索、分析、编码。
- **AI Agent（内部顾问 / 执行者）**：负责决策、规划或任务执行。
- **外部顾问（External Agent / Consultant）**：可调用外部能力补充系统。
- **支持工具（Ops / Support）**：定时任务、监控、日志、消息调度等。

---

## 系统架构
```
Company
├ Departments
│ ├ Research
│ │ └ Employees & AI Agents
│ ├ Engineering
│ │ └ Employees & AI Agents
│ └ Ops / Support
│ └ Scheduled Tasks & Tools
├ Employees / Skills
└ External Agents / Consultants
```

### AI Agent 分类

| 类型 | 归属 | 职责 |
|------|------|------|
| 执行型 | 部门内部 Agent | 使用技能完成任务，如 Research Agent、Engineering Agent |
| 顾问型 | 内部/外部顾问 Agent | 任务规划、决策建议、风险评估 |
| 调度型 | Planner / Manager Agent | 分配任务、调整优先级、监控绩效 |
| 支持型 | Support Department | 定时任务、日志、监控、公共工具 |

---

## 任务执行流程

1. **用户提交任务**
2. **CEO / 最高决策层** 分析任务，判断优先级与资源需求。
3. **内阁 / Planner Agent** 拆解任务，生成子任务。
4. **部门分配** 子任务给合适的部门。
5. **部门内部调度** 分配子任务给员工 / 执行型 AI Agent。
6. **技能执行** 员工 / Agent 调用具体 Skill 执行任务。
7. **结果汇总** 部门经理 / AI Agent 汇总任务结果。
8. **反馈用户** 系统返回最终结果。

---

## 绩效规则（Performance Rules）

系统根据 **员工 / AI Agent 完成任务的效率和质量** 打分，规则如下：

1. **任务完成率（Completion Rate）**
   - 完成任务按时率占比
   - 分数范围：0 ~ 1

2. **任务质量（Quality Score）**
   - 结果正确性、准确度、完整度
   - 分数范围：0 ~ 1

3. **资源消耗（Cost Efficiency）**
   - 任务消耗时间/计算/资源成本
   - 公式：`Efficiency = 1 - (ResourceUsed / ExpectedResource)`

4. **协作能力（Collaboration Score）**
   - 对跨部门任务或多 Agent 协作任务的贡献
   - 分数范围：0 ~ 1

5. **综合绩效评分**
   ```text
   PerformanceScore = 0.4 * CompletionRate
                    + 0.3 * QualityScore
                    + 0.2 * Efficiency
                    + 0.1 * CollaborationScore