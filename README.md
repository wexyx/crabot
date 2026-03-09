# crabot

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