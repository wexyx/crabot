# AdminAgent：对话是唯一管理入口

```text
Web 可视化 REPL ─┐
                 ├─ 对话/事件/确认 ─ AdminAgent ─ 管理 Skill + Tool ─ Core
crabot 终端 ──┘                                               │
                                                        群聊 / 业务 Agent
                                                               │
                                                       业务 Skill + Tool
```

两套能力包复用 AgentRuntime、Tool trait、Inventory、SkillCatalog 和事件持久化，不复制 Harness 或 Tool 调用逻辑。AdminAgent 支持 Crabot、Codex、Claude 和测试用 Mock。CLI Provider 通过共享 ToolRuntime 的 JSON 工具调用循环调用同一套 management 注册表，不另建管理协议。运行目录是独立临时目录，不是业务工作目录；工厂固定 Codex 为 read-only、Claude 为 plan，不再套 Crabot 外层系统沙箱。CLI 的认证文件读取仍需要目录策略授权，或预先提供对应 CLI 的环境认证信息。

缺少或无效配置时，终端启动自动进入配置向导；运行后 `/admin-config` 和 Web 的「AdminAgent 配置」可切换 Provider。配置保存在数据目录下的 default-agent.json（0600，明文密钥），环境变量优先。基础配置只允许用户操作，不注册为模型工具。切换时拒绝存在活跃管理任务的情况，保存成功后新消息使用新配置；历史不删除。无终端时配置错误返回非零退出码。

## 文件职责

- `core/`：与 HTTP/终端无关的业务入口、项目作用域、确认、消息调度。
- `management/context.rs`：注入固定项目的 Core 能力；不读取模型传入的 project_id。
- `management/tools/`：一个能力一个 Tool 文件，宏声明 management 作用域，自动注册。
- `management/skills.rs`：独立管理 Skill 仓库与版本检查。
- `management/service.rs`：AdminAgent 生命周期、会话并发限制和流式运行。
- `management/session.rs`：事件持久化；进程恢复不自动重放已执行工具。
- `management/execution.rs`：独立接收模型流，40ms 合并文本；不会等待每个片段写盘。
- `management/journal.rs`：未提交事件缓冲和实时通道；落盘后释放缓冲，消费者落后时读取日志。
- `management/writer.rs`：200ms 文本批量提交，工具边界提前刷新，最终事件先追加刷盘，再更新状态索引。
- `management/history.rs`：合并历史碎片与去除重复完整回答，不用模型摘要丢弃历史内容。
- `http/admin.rs`：薄对话传输适配器，没有执行任意管理工具的端点。
- `http/repl.rs`：群聊只读视图与消息通道。
- `cli/`：文本输入、导航、输出和人工确认，无管理命令分发。
- `http/server.rs`：可选监听器，生命周期独立于业务和 AdminAgent。
- `app/application.rs`：配置完成后装配和启动；不承担业务操作。

原有 A2A 策略和路由模块继续复用，尚未把全部旧内部模块拆成独立 core crate。

## HTTP 交互协议

配置了 ADMIN_TOKEN 时，下列接口需要 `x-admin-token`。默认空口令只允许本机回环服务，并检查 Host/Origin，不能匿名对公网监听。

| 接口 | 用途 |
|---|---|
| GET /v1/repl | 项目列表及 AdminAgent 状态 |
| GET/PUT /v1/admin-agent/configuration | 用户专用基础配置；GET 不返回密钥；PUT 为环境变量名到字符串的映射，省略 MODEL_API_KEY 保留，空字符串清除 |
| GET /v1/repl/{project}/groups | 群聊列表 |
| GET /v1/repl/{project}/groups/{group}/sessions | 群话题历史 |
| POST /v1/repl/{project}/groups/{group}/messages | 发群聊消息，content / 可选 previous_session_id |
| GET /v1/repl/{project}/sessions/{session} | 群聊持久化事件 |
| GET /v1/repl/{project}/sessions/{session}/events?after=N | SSE，从序号 N 之后恢复 |
| POST /v1/admin-agent/{project}/sessions | 创建管理对话 |
| GET /v1/admin-agent/{project}/sessions | 管理对话列表 |
| GET /v1/admin-agent/{project}/sessions/{id} | 状态和历史 |
| POST /v1/admin-agent/{project}/sessions/{id}/messages | 自然语言 content；异步接受后写入事件 |
| GET /v1/admin-agent/{project}/sessions/{id}/events?after=N | 内存实时流 + 历史回放，进程内支持 Last-Event-ID |
| POST /v1/admin-agent/{project}/sessions/{id}/interrupt | 打断管理会话 |
| GET /v1/admin-agent/{project}/approvals | 未完成的人类确认请求 |
| POST /v1/admin-agent/{project}/approvals/{id} | 人确认：allow 布尔值，单次生效 |
| GET/POST /v1/workspace/approvals[/{id}] | 宿主目录访问审批 |

事件包含递增 seq 以及 user / text_delta / tool_started / tool_finished / context_checkpoint / completed / failed。确认执行结果单独持久化，不自动发起额外模型调用；用户可以继续询问当前状态。管理对话单会话串行，最多 8 个并发会话；每项目一个固定管理聊天；历史写入知识索引，模型上下文读取有大小上限，完整历史保留在索引中。

活跃文本可能先展示、后批量持久化；硬崩溃可能丢失尚未提交的尾部，进程重启后应先重读完整会话历史并重置客户端游标，不复用崩溃前的临时 seq。完成通知仅在终态持久化成功后发送。磁盘故障会报告失败，不伪装完成；修复存储并重启后再继续受影响会话。正常中断会取消模型执行并刷新已接收文本。SSE 建连立即发送注释帧，不等待第一条消息或心跳。

内置管理指南直接装入上下文，不再强制每次用 find 加载指南。简单问候直接回答，仅需要当前节点数据时才查询工具；其他 Skill 仍按相关性读取。

A2A 保留 /v1/client/register、connect、events、control-results 等机器协议，用一次性邀请注册 AK/SK，然后子节点主动建立连接。它们不是恢复旧管理 CRUD 的后门。Web 和 CLI 不经这些接口管理本机。

## 管理工具与权限

管理工具覆盖业务 Agent 启停、发现子树、建群/完整策略更新/重新配置子群、默认模板、群聊派发/结果读取、独立 Skill 管理、项目创建、邀请、上游挂载和 Web 启停。

停止业务 Agent返回待确认动作；挂载会明确展示对上游授予的整个项目子树控制权。确认端点不在 ToolRegistry 中。请求绑定项目、具体参数、有效期和一次性状态，两个界面竞争确认只能有一个成功。目录确认亦不由模型自批。

当前仍为个人/可信节点管理员模型，不是多租户 RBAC。所有持有管理口令的用户都能切换项目；项目隔离限制的是会话和工具数据上下文，不是不同管理员账号之间的授权。运行状态和文件存储有规模上限；没有宣称任意第三方模型不会受到提示注入影响。

执行器直接启动本机进程，工作目录不构成安全边界。操作确认、白名单、取消和进程组清理仍保留；获准的脚本可以访问当前系统用户有权限的目录外文件。
