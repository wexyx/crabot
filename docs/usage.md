# Crabot 使用与配置

安装和架构见 [README](../README.md)。

文件/图片附件、智能压缩和按行回查见 [附件与上下文](chat-files-and-context.md)。

## Web 与 CLI

Web 使用 Vue 3 + Element Plus。终端与 Web 共享同一应用层、Agent、项目、日志和权限审批，不依赖彼此转发管理命令。

### 项目就是聊天群组

每个项目配置成员、角色、工作目录和协作策略：

| 模式 | 行为 |
| --- | --- |
| 简单聊天 | 有且仅有一个 Agent |
| 讨论模式 | 按职责发言，无关成员让出；有结论且其他成员确认或让出即结束，轮次仅为上限 |
| Leader 模式 | Leader 按成员角色分配任务并汇总 |
| 接力 | 按指定顺序、随机或协商优先级选择 Agent；额度不足时交给下一个 |

项目名称可留空，首次消息生成初始标题。列表「⋯」支持重命名和删除。删除采用软删除：从列表移除、禁止继续发送，本地历史日志保留；不删除 Agent。运行中的任务需先停止。

消息按连续聊天展示日期、时间和 Markdown。向上滚动加载较早消息；执行者与进度不拼进回答正文。工具调用可展开查看详细参数与结果。

### Agent 管理

「管理」中维护本地 Agent、虚拟 Agent 和组网。默认 Agent 不能删除；普通本地 Agent 编辑、删除前先停用，被项目引用时不能删除。停用只停止接收新任务，不打断已经开始的任务；取消任务请用聊天中的停止生成或 /interrupt。

远端 Agent 用「远端」标记，仅可查看和测试，不可在本节点改配置。IP:端口标注为连接来源，可能是 NAT / 代理后的地址，**不是远端 Web 管理地址**。

编辑角色与测试聊天使用右侧抽屉；测试会话单独持久化。远端目录在后台发现，不阻塞本地 Agent 操作。

### Claude 与 Codex 启动命令

首次启动和 `/agent-config` 使用同一配置向导：运行器、模型厂商、协议通过 ↑/↓ 选择，Enter 确认，Esc 取消。服务地址、模型名、启动命令、API Key 和环境变量仍可自由输入；已有配置会预选当前值。OpenCode 模型目录可用时支持选择模型或切换为手动输入。非交互输入和 `TERM=dumb` 使用编号列表，可填写序号或原始值；密钥不回显、不进入命令历史。

选择外部运行器后，确认启动命令时检查可执行文件及执行权限，支持 PATH 查找和带参数的自定义路径。未安装时显示官方安装说明，停留在当前步骤；可在另一终端安装后重新检测，或填写绝对路径。不会自动执行下载脚本，也不会把“程序存在”视为“已经登录”。安装参考：[Codex](https://learn.chatgpt.com/docs/codex/cli)、[Claude Code](https://code.claude.com/docs/en/setup)、[OpenCode](https://opencode.ai/docs/#install)。

Claude / Codex 的运行器配置支持启动命令，例如 `claude`、`claude --model sonnet`、`codex --model 模型名`，也支持 `"/带空格的路径/claude" --model sonnet`。Web 与 CLI 配置向导均可填写，仍保存在兼容原配置的 `CLAUDE_BIN` / `CODEX_BIN` 字段中。程序按 PATH 查找，参数按引号拆分，不执行 Shell 展开、管道或重定向；流式输出、任务提示词、执行权限和会话参数由 Crabot 添加，请勿重复填写。

### Agent 环境变量

默认 Agent 和普通本地 Agent 的配置表单都有“环境变量”键值表，支持新增、修改和删除；CLI 配置向导支持填写 JSON，例如 `{"ANTHROPIC_API_KEY":"...","ANTHROPIC_BASE_URL":"https://your-endpoint","LANG":"zh_CN.UTF-8"}`，回车保留、`-` 清空。启动文件也可使用 `AGENT_ENV_JSON`。

变量仅注入当前 Agent 及其工具进程，不修改服务器的全局环境；配置值优先于透传的宿主变量。Crabot 的模型配置也可通过 `MODEL_*` 环境变量覆盖，并支持独立 HTTP/HTTPS 代理。`LANG` 默认跟随启动环境，仅影响子进程区域设置，不限制 Agent 回复语言。

值不在配置查询中回显，编辑时未改动的值保留；本地明文保存，请妥善保护实例目录。`HOME`、`TMPDIR`、`CODEX_HOME`、`CRABOT_*`、`AGENT_*` 等运行目录/控制变量由宿主管理，不能通过 Agent 配置覆盖。每个 Agent 最多 64 个变量；值不做 Shell 展开。

### Tool 与 Skill

定义统一维护，管理能力与项目能力隔离。内置工具只能启用/禁用；非硬编码外部命令与 Skill 支持新增、编辑、上传、删除及测试。

每一行有「测试运行」和「配置范围」：
- 全局只列本地 Agent；选择项目后仅列该项目的本地成员。
- 不在 Tool / Skill 配置与测试中列远端 Agent；远端能力在远端节点维护。
- Agent 表格直接开关启用，自动保存，下一轮生效。
- 所有项目设置的是 Agent 默认值；单个项目设置可覆盖它。底层仍兼容全局、项目、Agent、项目内 Agent 的层级规则。

启用能力不是授权。命令、目录外访问、Python 仍受宿主策略和人工审批约束；管理 Agent 不能批准自己的请求。

### 执行权限

本地 Agent 支持三档，默认 `ask`，只由 CLI/Web 人工入口修改，不注册为管理 Agent 工具。权限保存在本节点该 Agent 的配置中，作用于该 Agent 的所有项目，不传递给远端 Agent。管理聊天独立保留审批，配置更新只影响后续任务；降低权限不会终止正在运行的任务，必要时先停止生成。

- **请求批准（ask）**：Crabot Shell 命令每次询问。
- **帮我批准（auto）**：使用当前 Crabot 实例全局命令白名单，默认约 50 条常见只读、目录查询、版本及受限 Git 查询命令。匹配完整命令和参数，支持 `*`（任意字符，含多个参数）和 `?`（单个字符），例如 `git status *`、`cat *.md`。可执行文件名必须明确；仍拒绝管道、重定向及命令替换。宽泛规则可放行脚本和危险参数，请谨慎配置。自定义脚本命令依然可能危险，请自行核对。
- **完全访问（full）**：明确确认后取消执行确认，可访问宿主当前用户有权限的文件和网络，包括目录外及凭据。仍不提供系统管理员权限，不自动启用被禁用的 Tool/Skill/Python。仅用于可信任务。

Web 在单本地 Agent 项目输入区域切换，完全访问有独立风险确认框。CLI：

```text
/permissions                         查看当前单 Agent 项目权限
/permissions default                 查看指定 Agent
/permissions default ask
/permissions default auto
/permissions default full --confirm-full-access
/allowlist                            查看全局白名单
/allowlist add cat README.md
/allowlist remove cat README.md
/allowlist reset                      恢复默认列表
/allowlist clear                      清空列表
```

普通读写 HTTP 接口：`GET/PUT /v1/repl/{project}/agents/{id}/permissions`；写入需 `mode`、`expected_version`，full 另需 `confirm_full_access:true`。沿用本机管理接口的信任边界，请勿对不可信本地程序开放管理端口；变更有审计记录。普通 Agent 配置保存不能注入权限字段。

**原生运行器边界**：Codex/Claude 非交互命令的逐工具授权协议尚未转接。ask/auto 下分别保留 Codex read-only / Claude plan，需执行命令时走 Crabot 注册工具及审批；不能转接的原生操作会拒绝，不会默认批准。full 下使用 Codex danger-full-access / Claude bypassPermissions。所有模式均不再套 Crabot 外层系统沙箱。此设置不解决模型账号登录认证问题。

授权与命令结果在 Web 输入框上方展示；默认只展示操作描述和目标，完整命令可展开核对。白名单在「管理 → 权限白名单」独立页面维护，GET/PUT /v1/permissions/allowlist，写入包含 command_allowlist 字符串数组和 expected_version。所有本地 Agent 共用，配置持久化并记录审计，不提供给模型自我修改。

命令请求支持「允许一次」「本对话允许」「拒绝」。本对话允许仅跳过当前项目聊天后续命令确认，不扩大目录访问权限；重置上下文或重启进程后失效，不影响其他聊天和管理操作。

Python 和命令执行保留文件系统沙箱，网络固定使用宿主网络，旧执行 profile 的 network:none 不再用于它们；常见 HTTP(S)/ALL/NO_PROXY 环境变量会透传。工作目录不是副本，修改直接落在本机文件上。原生 Codex/Claude 的运行限制仍遵循各自权限模式。

Agent 可配置回复要求（默认简洁、结论先行），作为每次调用的 Agent 提示词，不是硬性字数截断。群聊按 Agent 与 invocation 分开发言，工具事件不再作为回答文本输出；原有记录保持不变。

组网仅对已有明确 Web 地址提供跳转，不猜测下游来源端口，不提供手动管理地址设置。当前无认证管理接口仍只允许 loopback；跨机管理需本地转发或后续受认证的代理机制。

### 上下文与压缩

Web 聊天右上角「新上下文」与 CLI `/new` 使用同一分界机制：保留项目成员、策略和全部原始记录，但后续请求不再携带分界前的聊天内容。管理聊天也支持；执行中需先停止或等待。分界持久化，重启后仍生效。`/history` 仍只展示历史，不撤销分界。

每个 Agent 的上下文独立构建：最后一次摘要＋摘要覆盖序号后的全部聊天记录，没有摘要则读取重置点后的记录。各 Agent 按自己的上下文长度触发压缩；不再统一截取最近 N 轮。`compact` 触发宿主生成摘要并保存到索引；`find(target=history)` 和 `find(target=history)` 可回查原始内容。

管理与项目共用 `compact`（通过 `find(target=tool, query=compact)` 解锁；只做上下文管理，不读日志）：
- 不带参数或 `strategy=summary, summary=...`：Agent 自行总结并提交需要保留的决策、约束、进度与历史引用。
- `strategy=recent`：仅保留原始任务与最新完整工具轮次，较早工具过程按需用 `find` 回查。

Agent 配置中不再提供压缩策略选项。压缩在完整工具轮次结束后应用，原始任务、系统规则、当前工具调用与结果保持完整，原始日志不覆盖；模型需要更多细节时再次调用同一工具。

Crabot Harness 仍保留 `CONTEXT_MAX_TOKENS` 长度上限（默认 65536），包含工具定义并预留输出空间。超限时宿主执行本地有损摘录或明确报错，不依赖模型自行发现溢出，不额外请求模型生成摘要。Codex/Claude 自身的上下文上限仍由各自 CLI 管理。

### CLI 常用操作

```text
/help                       查看分类帮助
/manage 或 /admin           进入管理聊天
/back                       返回上一个聊天
/project                    列出项目
/project GROUP_ID           进入项目
/history                    加载当前聊天历史
/new                        新上下文，保留当前项目与历史
/agent list                 查看本地与远端 Agent
/agent test PATH            测试 Agent
/agent-config               修改默认 Agent 配置
/connections                查看上游与接入下游
/connect URL                申请连接上游
/disconnect upstream NAME   断开上游；downstream 操作接入下游
/reconnect upstream NAME    手动尝试连接；downstream 解除接入限制
/server start 8787          启动 Server
/server stop                只停止 Server
/interrupt                  中断任务
/tools [序号]               查看工具调用详情
/exit                       退出
```

CLI 启动不自动打印历史，使用 `/history` 恢复。`/chat`（或 `/sessions`）打开聊天选择器，第一项是「＋ 新建聊天」，下面是已有聊天。↑/↓ 选择，Enter 进入，Esc 取消；新建会创建使用默认 Agent 的独立简单聊天。管道模式按编号选择，`/cancel` 取消。`/chat ID或名称` 保留为快捷入口，支持唯一 ID 前缀；重名或前缀重复时请填写完整 ID。`/history ID或名称` 进入并恢复记录，`/new` 只重置当前上下文，不删除聊天。`/manage` 进入管理，`/back` 返回。

输入 `/` 打开带说明的命令菜单，继续输入可筛选，↑/↓ 或 PgUp/PgDn 选择，Tab 补全；Enter 对不完整命令先补全，再按一次执行。没有命令菜单时方向键切换输入历史。鼠标保留终端原生滚动、拖选复制。Ctrl+C 优先中断任务或清空输入；管理会话空闲、输入为空时返回上一个聊天。Ctrl+P 仍用于审批，Ctrl+D 退出。

`/agents` 始终查看全部 Agent，`/members` 查看当前项目成员。`/project` 保留创建和配置功能，`/agent` 保留增删改查及测试。`/help commands` 列出全部主要命令；复杂配置可使用 Web 或管理对话。旧别名 `/admin`、`/admin-config` 仍可使用，但不在默认菜单重复展示；旧存储空间命令 `/namespace` 已移除，用 `/chat` 或 `/manage` 替代。

## 去中心化组网

每个节点既能提供 Agent，也能主动连接上游。下游主动建立 HTTP + SSE 长连接，因此不需要暴露本地公网端口。不同局域网可共同挂载一个公网 Crabot / ProxyAgent。

连接需本机确认授权；自动注册取得 AK/SK，凭据保存于各自节点。上游可发现、调用后代能力及管理授权范围内的子群策略，不能修改远端 Agent 定义。节点不能借此向上访问父节点或兄弟节点的数据。

组网页面展示实际连接配置和状态，不是日志：
- 分别展示上游和接入下游；两端都可断开。
- 每次只尝试连接一次，失败或断线后不自动重试，需手动操作。
- 已连接地址重复提交不会建立重复连接。
- 断开状态持久化；失败连接可删除。下游解除限制后，需要对方手动重试。
- 断开不自动取消已下发任务；聊天和任务日志保留。

## 数据、安全与配置

配置、凭据、策略使用本地状态文件；聊天记录写入按项目隔离的知识索引（LadybugDB），不再写入 JSONL 日志文件。目录锁防止多进程写同一个实例。备份前停止实例，再备份完整数据目录。

`default-agent.json` 保存默认运行器/模型配置；密钥本地明文保存，Unix 文件权限为 0600。不要提交、公开或同步活动实例目录。

无需 AdminToken。管理路由仅允许实际回环连接并校验 Host/Origin；公网仅用于 A2A。远程管理使用 SSH 本地端口转发，不要把管理路由通过反向代理开放到公网。

项目可覆盖工作目录和文件工具的目录外访问策略（deny / ask），不能扩大宿主配置允许的文件工具权限。Shell/Python 在工作目录直接运行，具备当前系统用户的文件和网络权限；cwd 不是安全边界。操作确认与命令白名单仍然有效。

模型环境变量：`MODEL_PROVIDER`、`MODEL_NAME`、`MODEL_API_KEY`、`MODEL_BASE_URL`、`MODEL_API`、`MODEL_SYSTEM_PROMPT`。`MODEL_SYSTEM_PROMPT` 留空使用实例 `conf/agent.md` 覆盖；没有覆盖则使用当前版本默认模板。CLI 初始化只读取和校验，不复制文件，可通过 Web「管理 → 系统提示词」、`/prompts` 或同名文本文件覆盖；恢复默认会删除覆盖，详见[系统提示词](system-prompts.md)。显式环境变量在重启时优先于保存配置。源码和安装版统一读取 `<实例目录>/.agent.env`（默认 `~/.crabot/.agent.env`）；若运行目录存在 `.agent.env`，其同名配置覆盖实例文件。启动参数优先于显式环境变量，显式环境变量优先于文件。配置文件仅支持字面量 `KEY=value`，不执行 Shell 或变量替换；相对目录基于配置文件所在目录。`--name` / `--data-dir` 先选择实例，再读取配置；实例文件不能通过 `CRABOT_DATA_DIR` 重定向自身。运行目录配置中的 `CRABOT_DATA_DIR` 可以选择实例，仍受显式参数/环境变量覆盖。交互保存的模型配置 `default-agent.json` 优先于文件中的模型默认值，避免旧模板覆盖已保存的配置。

### 本地持久化与内置 Skill

聊天记录写入按项目隔离的知识索引（`<数据目录>/knowledge/<project>/graph.db`），每条事件带序号与完整 payload，摘要节点按 `(chat, agent)` 隔离并标记其覆盖的记录；实例状态仍写入 `state.jsonl`，一行对应一次事务，仅包含变化的记录，不再在每次操作时重写整份 `state.json`。启动时回放日志，兼容旧 `state.json` 基线；旧文件请保留，不能单独删除。JSONL 中间损坏会拒绝启动，崩溃留下的未完成末行会恢复到最后一次完整提交。旧版本遗留的 `chats/` JSONL 目录不再读写，可自行清理。

内置 Skill 源码在 `skills/system/{business,management,shared}/<skill>/`，`shared` 中的 Skill 同时提供给项目与管理 Agent，仍分别遵循各自启用范围。每个目录包含 `SKILL.md`、`skill.json`（描述和默认启用状态），可附带 `scripts/`、`references/`。`scripts/package-release.sh` 自动将其放入发布包 `skills/system`；`install.sh` 随程序复制到版本目录，启动器通过 `CRABOT_SYSTEM_SKILLS_DIR` 定位。也可显式指定这个环境变量覆盖来源。

系统 Skill 在工具库中只读，可通过已有生效范围规则启用/禁用；安装升级不写入用户 Skill 或实例数据。内置依赖安装、浏览器自动化、OCR Skill 只提供按需工作流，不会在安装 Crabot 时自动安装 Homebrew、浏览器或 Tesseract，也不会绕过执行确认。

新建或上传的 Skill 保存在当前实例的 `skills/user/` 下：默认是 `~/.crabot/skills/user/`，命名实例是 `~/.crabot_<别名>/skills/user/`；使用 `--data-dir` 时跟随指定的数据目录。项目与管理 Skill 分别放在 `business/`、`management/` 中，正文 `SKILL.md` 和脚本、参考资料都以独立文件保存。内置 Skill 仍留在发布包目录，不会复制到实例目录。

Skill 的运行依赖与源码分开保存。浏览器依赖统一位于 `<实例数据目录>/runtime/browser-automation/.runtime/`，`install.mjs` 和 `browser.mjs` 共用该目录。已有依赖跨重启和升级复用；Puppeteer 或匹配的浏览器缺失时自动进入构建流程，仍遵循当前执行授权。不会搜索或修改旧 release、源码仓库或 `~/.browser-skill` 中的依赖；Node.js 与 npm 需提前安装。

每个用户 Skill 按“名称与标识 / 保存版本”分目录，保存后在状态日志中记录当前目录引用，不再把正文与脚本写入该条日志。这样保存失败不会覆盖上一版；旧版本文件保留用于恢复，删除 Skill 后不会继续加载它。推荐通过 Web 编辑，直接修改当前版本的文件也会在下次加载 Skill 时生效。旧版已存于日志的自建 Skill 仍兼容读取，下次保存时会写入实例目录。

### 创建 Skill、记忆与默认能力

内置 `skill-creator`（创建 Skill）可以通过 `skill` 工具列出、读取和保存用户 Skill，直接注册到当前实例的共享 Skill 库，而不是仅生成一个未注册的文件夹。新 Skill 默认停用，请在 Web 的 Skill 库中启用并选择生效范围；Agent 不能通过创建 Skill 自行扩大执行权限。项目 Agent 只能维护项目 Skill，管理 Agent 只能维护管理 Skill；修改已存在的 Skill 需要最新版本号。

每次任务开始，已启用的内置工具 schema、内置 Skill 正文与脚本目录直接提供给模型，无需先调用 `find`。用户 Skill 先提供名称和描述，正文按需加载；外部命令仍可通过 `find` 发现。被禁用的能力不会因为预加载重新开放。

`memory` Skill 用 `find(target=history,query="关键词")` 检索当前实例的不同聊天与项目。返回 `project`、`chat`、`seq`，可用同一工具指定这些坐标和序号范围读取原文。未提供查询词且未指定 project/chat 时，仍只读取当前聊天。存储和自动构建上下文保持按聊天、Agent 隔离；只有主动检索才跨聊天，不查询远程节点或其它实例。部分索引不可用时通过 `unavailable` 明确报告，不影响其它索引的结果。

长期记忆文件统一为 `<实例目录>/work/memory.md`，默认 `~/.crabot/work/memory.md`，命名实例使用 `~/.crabot_<别名>/work/memory.md`；shell 提供 `CRABOT_MEMORY_FILE`。有需要时通过正常命令授权读写，不把聊天全文反复写入此文件。旧的 `~/memory.md` 不会被自动搬动或导入，以免混入其它程序的数据。

Web 和 CLI 的实时工具状态包含简短参数，例如 `shell · python report.py`、`find · skill : "browser"`；OpenCode 的代码包装调用显示 `execute · find · tool : "shell"` 等内部操作，不把整段代码塞到状态行。内容过长截断，敏感命令参数遮盖，完整参数在详情中查看。进程会话操作显示「读取进程输出 / 发送进程输入 / 停止进程」，不把输入的密码或验证码放到状态行。

### 讨论中的 Agent 协作

讨论模式下，用户 `@A` 指定主责 Agent，不会把其它成员移出本轮可协作名单。A 可以另起一行用 `@B 具体问题` 请教群内成员；B 回答后，调度器让 A 继续处理原任务。B 也能继续请教其它成员，各条回复仍以独立 Agent 消息展示。

协作不转移主责，也不会扩大任何 Agent 的执行权限。只有当前群成员可被调用，存在同名成员时使用完整路径（例如 `@node/b`）。代码块、引用、普通正文中的名字不触发协作。有待答问题时不宣告共识；重复追问与协作次数有上限，触限会明确提示尚有问题待处理。
