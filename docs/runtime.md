# Runtime 工厂

运行时通过统一 AgentRuntime trait 执行。RuntimeFactory 是具体实现的组装入口，AgentNode 不再按 provider 分支执行。

```rust
use agent_runtime::RuntimeFactory;

// 显式选择类型；从对应的既有环境变量读取配置。
let runtime = RuntimeFactory::create("mock")?;
let answer = runtime.run("hello", &mut |delta| {
    println!("{delta}");
}).await?;
```

RuntimeFactory::from_env() 根据 AGENT_PROVIDER 创建；默认仍为 mock。RuntimeFactory::from_config(RuntimeConfig) 接受显式配置，可用于多个独立实例和不修改全局环境的测试。创建后配置固定，每次 run 的会话状态独立，实例可重复使用。创建不启动 CLI、不调用模型，执行发生在 run 时。

目录职责：

- lib.rs 和所有 mod.rs：只声明模块和导出符号，不放类型定义、构造或执行逻辑。
- runtime.rs：仅定义 AgentRuntime 接口及回调、Future 类型。
- managed_runtime.rs：所有 Provider 共用的 ManagedRuntime 生命周期实现及测试（过滤事件、统一终态）。
- events.rs：纯 RuntimeEvent / RuntimeErrorCode 事件协议和映射，不承载 Provider 实现。
- runtime_kind.rs：Provider 类型枚举和名称解析；crabot 为自研 Provider，builtin 仅为旧配置兼容别名。
- factory.rs：RuntimeFactory，集中选择和创建具体实现。
- config.rs：RuntimeConfig 和兼容配置导出；具体配置、默认值、校验归各 Provider 自己持有。配置含密钥，不实现 Debug。
- providers/provider.rs：统一 Provider 接口。mock/、crabot/、codex/、claude/ 是平级实现，均在自己的 runtime.rs 内封装私有状态与 new 构造方法，工厂不操作内部字段。
- providers/crabot/：自研 Harness。runtime.rs 仅适配 Provider；engine.rs 持有可复用依赖并控制任务总超时；run.rs 持有单次任务的历史、ToolSession 和循环；turn.rs 封装单轮输出与完成状态；client.rs 处理 HTTP/SSE 传输。protocol/ 下的 ProtocolFactory 选择 Chat、Responses、Anthropic 适配器，每种适配器负责工具 schema、请求认证、增量解析与结果回填。执行循环不判断 API 类型。统一使用 AGENT_PROVIDER=crabot；旧 builtin 仅为兼容别名。
- providers/codex/、providers/claude/：各自的 config.rs、runtime.rs、parser.rs 和单元测试，厂商协议不混用。
- providers/mock/：同样实现 Provider、经过相同工厂与生命周期包装；没有配置/协议需求时不创建空 config/parser 文件。
- errors.rs：共享 token/context 不足判定。
- skills/：与厂商无关的 SkillCatalog、资源校验和本机执行策略。部署与 API 见 [Skill 与本机执行](skills-and-sandbox.md)。
- tools/：统一 Tool 接口、宏注册工厂、运行时注册表和 ToolRuntime 桥接；Crabot 原生工具与 Skill 工具共用实现。见 [工具注册](tools.md)。
- agent.rs：原有 run / run_with_provider 的兼容入口，只转发给工厂。

现有 AGENT_PROVIDER、MODEL_*、HARNESS_*、CLAUDE_*、CODEX_*、AGENT_WORKDIR 配置继续有效。Claude 的默认 plan 权限、Codex 的默认 read-only sandbox、流式事件格式和进程 kill_on_drop 保持原样。

新增执行器时，实现内部 Provider，在 providers 下新增模块，在 RuntimeKind、RuntimeConfig 和 RuntimeFactory 中加入对应分支。工厂统一套上 ManagedRuntime，对外返回 AgentRuntime，业务调用方无需增加 provider 分支。当前是编译期工厂，不是动态插件注册系统。

## 标准事件

run_events(prompt, on_event) 输出 TextDelta、ToolStarted、ToolFinished、ContextCheckpoint、Completed、Failed。Crabot（自研 Harness） 输出工具生命周期。Codex 非文本 item 的启动/更新/完成、Claude assistant/user 记录保存为不透明 ContextCheckpoint（网关事件 agent.context），保留工具输入/结果等原始观察供续接使用，不伪造统一工具状态；其他未知事件忽略。厂商原始 type 不作为网关事件类型。

ManagedRuntime 在执行返回后生成唯一 Completed 或 Failed。CLI 会等待退出状态和 stderr，即使先收到成功载荷，非零退出仍判为失败。原始协议终态之后的增量忽略；文本最终快照不重复追加已流出的前缀。取消或丢弃执行 future 不承诺终态回调，上层超时仍由调用方报告。

返回值与终态事件表示同一个执行结果。AgentNode 只将非终态事件映射到进度 SSE，使用返回值发布已有 agent.message + agent.done 或 agent.error，避免双重完成。工具映射为 agent.tool.started / agent.tool.finished，content 为标准工具事件 JSON；旧组报告接口将其作为 working 内容传递。run 和 agent 模块保留文本兼容投影。

## 当前会话边界

CLI 仍然每次启动新进程（Codex 使用 --ephemeral），上下文由 Crabot 管理，不依赖厂商进程或 resume ID。本地执行器在真正出队时读取同项目、同会话的历史（用户消息、已收到的输出及标准工具事件），构建 prompt，再把运行状态和实际 prompt 写入 `.crabot/state.json` 的 runs 集合。运行中直接发送是排队追加，不会同时修改工作目录；需要改方向时先中断，再发送补充要求。

- `POST /v1/sessions/{id}/interrupt`：中断本地会话的在途及排队任务，保留历史。轮询取消周期 100ms，丢弃运行 future，CLI 使用 kill_on_drop。不是工具副作用回滚，也不保证 CLI 孙进程全部退出。远程节点 / 含远程成员的群没有取消确认协议，返回 409，不宣称远端已停止。
- `POST /v1/sessions/{id}/messages`，`{"content":"改成……"}`：同会话继续，自动恢复上下文。
- `GET /v1/sessions/{id}/runs`：读取持久执行状态和 prompt（需要管理认证）。
- `POST /v1/crabot/runs`，`{"project_id":"…","group_id":"…","previous_session_id":"前次运行 UUID","content":"继续并修改……"}`：同群话题续接，继承前次话题链上下文，创建新的执行快照。省略 previous_session_id 是新话题；跨项目/跨群不能续接；前次仍运行时拒绝。

Web 会话页提供“中断并保留进度”和“发送 / 继续”，群任务页面启动的话题也能继续；重启后打开已保存的群会话可恢复关联。重启将 queued/running 标为 interrupted，不自动重放工具，发送新要求后才继续。迟到的旧输出不会写入中断任务。全部数据仍为本地 JSON 文件，无数据库依赖。

边界：不是模型内部推理状态或原生 CLI 会话的完整快照，只能恢复 Crabot 已收到并持久化的内容；未输出、未接收或未落盘的数据无法恢复。部分输出不代表完成，恢复 prompt 提醒检查文件和工具副作用。上下文超过 512 KiB 明确拒绝，不静默截断；原始记录保留在文件中，可整理摘要后建新话题。普通 Web 展示的最近 500 条上限不影响执行上下文读取。旧 `/v1/groups` 群接口没有新增话题续接语义。

回归测试：cargo test --workspace --offline。CLI 测试用临时模拟可执行文件，模型协议测试用本地 HTTP 服务，不依赖真实 CLI 登录或模型额度。
