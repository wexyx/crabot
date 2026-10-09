# 统一工具与注册

Crabot 内置工具和 Skill 服务工具使用同一个 `Tool` 接口、`ToolDefinition` 描述、
`ToolRegistry` 分发与 `ToolFactory` 构造流程。参考 crabot 的 inventory 工厂收集方式，
不使用全局可变工具实例；每次构造携带当前工作目录、项目 Skill 快照和执行策略。

- Crabot：注册表定义转换成 Chat / Responses / Anthropic 原生工具 schema。
- Codex / Claude / Mock：ToolRuntime 提供 JSON 工具桥接；Mock 本身不产生智能工具决策。

Provider 通过 `handles_tools` 声明是否自行管理工具循环。Crabot 只运行自己的原生工具循环，不再外套 JSON 桥接，避免冲突指令和两层循环重复执行。
- 桥接只解析调用与驱动循环，不实现工具业务或权限判断。
- 项目内置基础工具仅保留 `shell`、`find`、`compact`。目录列表、文件读取、Python 和浏览器操作通过 Skill 指导 `shell` 完成；图片发布通过 `image-view` Skill 完成。管理作用域仍保留管理 Agent、项目、组网等宿主操作工具。
- Skill 仍是指令/资源包，不是可绕过授权的宿主插件。

## 按需发现

模型初始只能看到 `find` 一个入口，本节点注册的其他工具与 Skill 说明都不进入提示词。

询问「有哪些 Skill」时，模型调用 `find({"target":"skill"})`，无需猜关键词或提前知道 ID。返回当前 Agent 可用的 Skill ID、说明和文件名，不包含指令正文；`find({"target":"tool"})` 同样可以列出工具名称与说明。列表受每页数量限制，`truncated=true` 时保持查询条件并传 `offset=next_offset` 继续。`registered` 是可用总数，`matched` 是关键词匹配数，两者不同：关键词没有命中不代表未注册或缺少说明，应去掉 `query` 查看列表。显式选择 Skill 类别后，单独的 `skill` / `skills` / `技能` 关键词也按列目录处理。

- `find`：统一检索入口。`target=tool` 按 `name` 精确取一个工具，或用 `query` 关键词匹配工具名与说明，命中项在下一轮变为可调用工具（deep 及以上附带 schema）；`target=skill` 用 `query` 列出 Skill 的 id、说明与文件名；传入 `id` 则直接加载完整说明及脚本目录（`skills[0].directory`）；`target=history` 在本项目内跨会话检索历史，返回会话名、序号与摘要片段，背后是随每次写入增量更新的知识索引；`target=doc` 知识库尚未建设，调用成功并明确返回不可用，而不是报错让模型反复重试。`depth`（brief/normal/deep/exhaustive）控制返回数量，受运维上限钳制。
- `compact`：通过 `find(target=tool, query=compact)` 解锁，只做上下文管理，不再接收摘要文本。`strategy=summary`（默认）触发宿主驱动压缩：宿主把当前上下文窗口交给模型生成摘要，写入摘要索引（按 `(chat, agent)` 隔离），并把窗口压缩为摘要；`strategy=recent` 仅丢弃旧工具轮次。需要过去的内容一律用 `find(target=history)` ；不传 query 时按序号范围读取原文。

披露只影响「展示」，不改变「授权」：执行始终走完整注册表，被策略移除的工具既查不到也调不了。Crabot 原生循环每轮按已解锁集合投影；桥接每轮重建提示词头部，因此新解锁的工具下一次请求即可调用。Skill 正文只在 `find(target=skill, id=...)` 返回后进入上下文，不再预先注入。

历史搜索和顺序回查共用一个入口：

```json
{"target":"history","query":"部署方案"}
{"target":"history","chat":"会话 ID","after_seq":120,"before_seq":180,"limit":10}
{"target":"history"}
```

以上分别表示关键词搜索、按序号范围读取、读取当前会话最近记录。序号边界不包含端点；`chat` 缺省为当前会话。`query` 不与序号范围混用，结果仍受发现深度的每页上限约束，被摘要覆盖的原文也可以回查。

知识索引是聊天记录的唯一存储：每次持久写入（含完整事件 payload 与序号）直接落索引；摘要与历史记录都在其中，按 `(chat, agent)` 隔离。摘要索引与历史索引均为内部使用，不会主动暴露给模型；启动聊天时上下文窗口优先读取最后一次摘要，再读摘要之后的原始记录。旧 JSONL 日志文件不再读写。

## 属性宏自动注册

### Web 工具管理

「管理 / 项目」侧边栏分别提供管理工具、项目工具入口。项目工具按本地 Agent 隔离：内置注册工具可以启用/禁用，不可删除；外部固定 Shell 命令工具支持新增、编辑、启停、删除。外部工具使用同一 Tool/Factory 契约，不能覆盖内置名称，也不会插值模型参数。所有外部命令仍经过逐次人类确认与原生沙箱。

配置保存在当前实例的文件存储中，使用版本校验防止覆盖并发修改。禁用会同时移除下一轮的模型工具定义和执行入口；已经开始的轮次保留其工具快照。这里只控制 Crabot 注册工具，不控制 Codex/Claude 自带的私有工具。远端 Agent 的工具配置在其所属节点维护。

管理接口（沿用本机访问/管理员认证保护）：`GET/PUT /v1/repl/{project}/tool-config/{management|business}/{agent}`。管理 Agent 标识为 `admin`；PUT 参数为 `{expected_version, policy:{disabled:[内置工具名], external:[{name,description,command,enabled}]}}`。内置工具定义不允许从此接口修改。

Skills 入口同样按管理/项目区分，可创建、编辑文件与启停，内置管理指南只读。`GET/PUT /v1/repl/{project}/skills/{management|business}`；PUT 使用 `{expected_version,definition}`。这些编辑不自动授予脚本执行权限。

### 上传与试运行

项目工具可导入单个定义或定义数组的 JSON：`{"name":"show_status","description":"查看状态","command":"git status --short"}`。导入先预览，保存默认禁用，不能覆盖同名内置工具。这是固定命令适配器，不是上传执行 Rust 插件。

Skill 可上传 UTF-8 文件、文件夹或 `{id,description,files}` JSON 包；必须包含非空 `SKILL.md`，不支持 ZIP。最多 32 个文件，单文件 64 KiB、内容合计 256 KiB。路径穿越和重复文件会被拒绝。导入默认禁用，不继承脚本授权。

详情中的「运行测试」使用真实 ToolRegistry 和当前作用域配置；填入符合工具参数说明的 JSON，例如 shell 使用 `{"command":"pwd"}`。执行可能修改数据或联网，不是模拟预览。Shell 执行遵循命令审批；Skill 测试先加载指令和临时脚本目录，再通过 shell 测试选中的脚本，不自动授予权限。离开测试区取消尚未结束的测试；已经完成的副作用不会回滚。

测试接口：`POST /v1/repl/{project}/tool-config/{scope}/{agent}/tests`，参数 `{name,arguments}`；`GET/DELETE /v1/repl/{project}/tool-tests/{id}` 查询或取消。沿用管理员保护，返回任务状态与输出；最多 4 个并行测试、每次最多 240 秒、输出最多 64 KiB。结果保存在有界内存中，重启即清空，不写入聊天历史。

业务工具 `shell({"command":"..."})` 使用同样的属性宏/工厂注册。在当前 Agent 工作目录中运行 `/bin/sh -c`，每次都要求用户确认完整命令（不依赖不可靠的危险命令黑名单）。确认按当前权限模式授权执行；Shell 具有当前系统用户的文件与网络权限。CLI 权限面板和 Web 均可批准/拒绝；120 秒未批准会失败，取消任务撤销待审批请求。

命令继承宿主沙箱 profile 的网络和超时策略；限制输出、CPU、文件大小及文件描述符，取消/超时终止进程组。不会自动注入模型密钥或宿主环境凭据。不能保证批准后的命令不删除工作区文件或不联网；请核对命令。沙箱不可用时失败，不降级为裸执行。

参考 oxygen_middleware 的 bean_constructor / dynamic_event：扫描 inherent impl 的
new 与 execute，自动生成接口适配器与 inventory 提交代码。工具不再手写 impl Tool
或 register_tool!；名称、说明和参数 schema 都在属性中声明。

```rust
use agent_runtime::tools::{tool, ToolContext, ToolSession};
use serde_json::{Value, json};
use std::sync::Arc;

struct Echo;

#[tool(
    name = "echo",
    description = "Return the input",
    parameters = json!({
        "type": "object",
        "properties": {"message": {"type": "string"}},
        "required": ["message"],
        "additionalProperties": false
    })
)]
impl Echo {
    fn new(_context: Arc<ToolContext>) -> Option<Self> {
        Some(Self)
    }

    async fn execute(
        &self,
        args: &Value,
        _session: &mut ToolSession,
    ) -> Result<Value, String> {
        // schema 用于模型描述；实现仍负责验证参数。
        let message = args["message"].as_str().ok_or("message is required")?;
        if args.as_object().is_none_or(|fields| fields.len() != 1) {
            return Err("unexpected arguments".into());
        }
        Ok(json!({"message": message}))
    }
}
```

最小声明是 `#[tool(name = "echo")]`：description 默认等于 name，parameters 默认
为 object schema。生产工具应声明准确 schema。宏拒绝重复/未知选项、无效名称、
泛型 impl、trait impl 和缺少约定方法的实现；完整参数类型由 Rust 编译器检查。

约定 new(Arc<ToolContext>) -> Option<Self>；返回 None 表示当前项目不启用。
可读取 context.workdir() / context.skills()。execute 是上面签名的异步方法。
name 仅在属性定义一次，不需要维护工具名称列表。

宏在链接时由 inventory 收集构造器，Runtime 初始化时由 ToolFactory 自动创建并注册。
工具模块仍需在 mod.rs 声明并链接进二进制；这不是目录扫描或动态库加载。
agent-runtime 自己内部使用 `runtime = crate`；外部默认路径为 ::agent_runtime，
依赖重命名时可显式指定 `runtime = your_alias`。

## 运行时注册

```rust
let context = ToolContext::new(Some(workdir), catalog, policy)?;
let mut registry = ToolFactory::create(context)?;
// 对于运行时才获得的工具对象，仍可直接注册：
registry.register(tool_instance)?;
registry.unregister("unused_tool");
let runtime = RuntimeFactory::from_config_with_tools(config, registry.clone())?;
```

register 接受任意实现 Tool 的对象，拒绝重复名称，不能静默覆盖权限受控工具。
unregister 影响当前注册表；clone 是注册集合快照，运行中的 Runtime 不会被外部增删影响。
工具实例由 Arc 共享，因此实现者应将每次调用状态放入调用作用域，而非可变全局状态。
默认工厂会向模型提供项目 Skill 目录；自定义 from_config_with_tools 入口只注入工具定义，
使用者需要在任务上下文中提供需要发现的 Skill ID/目录。

动态注册是宿主 Rust API；Web 可维护固定外部命令定义，不支持上传宿主 Rust 代码执行。
宏注册新 Rust 实现需要重新编译；宿主已具备的适配器（例如将来 MCP 客户端）
则可以运行时构造并注册，不需要新增名称分支。

## 权限与兼容

工具名称为 1–64 位 ASCII 字母、数字或下划线，以兼容模型函数协议。
原生调用和 JSON 桥接直接使用相同注册名，不再接受旧点号别名。宏 scope 可为 business（默认）、management 或 shared；工厂先按能力包过滤，再调用构造器。
ToolDefinition 描述参数；工具实现负责反序列化和验证参数，注册表不替自定义 Tool
运行通用 JSON Schema 验证器。内置工具拒绝未知字段。

工具授权在实现内部：`shell` 统一走命令确认、白名单及任务取消；工作目录不是文件隔离边界。Skill 只是操作说明，加载 Skill 不授予执行权限。图片 Skill 的内部发布接口仅接受工作目录 tmp 中的图片，拒绝 URL 和逃逸路径。原生调用和 JSON 桥接使用同一注册表。
新增自定义工具是可信宿主代码，不自动获得沙箱隔离；不得将不可信代码注册为宿主 Tool。

## 验证

`cargo test --workspace --offline` 覆盖宏收集、动态注册/删除、重复拒绝、
快照隔离、Skill 加载、shell 授权与三个模型协议调用。
`node --test scripts/skills-smoke.test.mjs` 覆盖 CLI 桥接到 shell 的链路。
烟测通过 shell 执行真实 Python；模型和 CLI 协议使用本机替身。

## 统一能力库与四层规则

定义集中在本节点能力库维护，管理与项目能力保持隔离。新工具/Skill 默认停用，编辑一次影响所有引用位置。Web「项目工具库 / 项目 Skill 库」可切换项目与 Agent 查看最终状态。

启用设置为三态：继承、启用、禁用；优先级为 **项目内 Agent > Agent > 项目 > 全局 > 定义默认值**。Agent 层按本节点 client_id 跨项目匹配。启用不授予 Python、目录外访问或 Shell 确认权限，也不控制供应商 CLI 私有工具。

旧定义不复制、不删除，集中显示并保留原项目/Agent 的默认绑定范围。同名但不同内容的旧定义分别展示；在同一执行范围启用重名定义会报错，不会静默覆盖。旧工具/Skill 管理接口仍可编辑原定义，但 Web 新入口使用统一接口。

- GET/PUT `/v1/repl/{project}/capabilities/{management|business}/{tool|skill}`：列出全节点定义与生效状态；PUT 为 `{id?,expected_version,definition}`，删除为 `{id,expected_version,deleted:true}`，内置不可编辑/删除。
- PUT 同路径 `/bindings?agent=CLIENT_ID`：`{id,layer,enabled,expected_version}`，layer 为 global/project/agent/project_agent，enabled 为 true/false/null（继承）。
- GET 添加 `?agent=CLIENT_ID` 可查看四层来源；未选 Agent 时只展示前两层。
- 所有接口沿用管理员认证；配置持久化，下一轮执行加载，不修改进行中的执行快照。
