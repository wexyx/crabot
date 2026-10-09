# 启动与交互

1. 安装 Rust；需要 Web 时先运行 `pnpm install && pnpm web:build`。
2. 直接启动 `./crabot`，缺少配置时按向导选择 Crabot/Codex/Claude。也可参考 .env.example 使用环境变量或 .agent.env；不要在普通 AI 对话里发送模型密钥。运行后用 `/admin-config` 或 Web 基础配置面板修改。
3. 运行 `./crabot --workdir /absolute/path --outside-access ask`，直接向 AdminAgent 描述需求。
4. 想看 Web 时告诉 AdminAgent“启动 Web 界面”；或直接 `./agent-node start` 以 Web 服务模式启动。
5. Web 默认空口令，本机打开即进入管理聊天。启动时可加 `--admin-token TOKEN` 或设置 `ADMIN_TOKEN`；监听非回环地址必须设置口令。旧 `admin-token` 文件不再自动启用。

无需数据库、容器服务或独立沙箱进程。默认数据目录 .crabot；不同节点指定不同 CRABOT_DATA_DIR，不能并发共享文件。

## 终端交互

真实终端使用内联 REPL：已完成的回答进入终端原生滚动区，底部显示输入区和运行状态。输入和输出不会互相覆盖。青色区分用户输入，黄色表示工具调用/等待，绿色表示完成，红色表示错误；设置 `NO_COLOR=1` 关闭颜色。正文沿用终端默认颜色。

- Enter 发送，Alt+Enter 换行；支持多行粘贴、中文编辑。
- 输入 `/` 打开命令菜单，↑/↓ 选择，Tab 补全；未打开菜单时 ↑/↓ 浏览输入历史。
- Esc 或 Ctrl+C 打断当前任务；空闲时 Ctrl+C 清空输入，再按一次退出，Ctrl+D 空输入退出。
- PageUp/PageDown 切换菜单选项或输入历史，Ctrl+L 清屏（不删除持久化历史）。
- 使用内联 REPL，不进入全屏、不捕获鼠标。滚轮与拖选复制由终端处理；↑/↓、PgUp/PgDn 切换输入历史，无 F2/F3 模式。工具默认一行，`/tools [序号]` 查看详情。启动不回放聊天；`/history` 恢复当前聊天，`/history admin` 或 `/history GROUP_UUID` 恢复指定聊天；聊天记录与模型上下文始终保留。
- Web 和交互终端渲染 Markdown；终端保留标题、列表和代码块的可读格式，非交互管道保留原始文本。
- `/agent-config` 在界面内配置模型（兼容 `/admin-config`）。运行器、厂商、协议通过方向键选择；密钥遮罩显示且不进入输入历史。首次启动使用同一组配置选项。

管道输入、重定向或 `TERM=dumb` 自动使用普通文本 REPL，供脚本使用。界面风格参考常见 AI CLI，并非完整复刻 Claude CLI。

AdminAgent 文本约每 40ms 合并推送，文件写入在独立任务中约每 200ms 批量执行；工具事件促使批次提前刷新，最终事件追加刷盘并更新轻量状态索引后才显示完成。强杀/断电可能丢失尚未提交的最后一小段文本；正常打断会刷新已接收输出。模型网络等待不由此优化消除。群聊记录同样写入知识索引，CLI/Web 按群 ID 读取增量。

## 组网与建群

在父节点的 AdminAgent 对话中申请一个子节点邀请码；在子节点对话中提供父节点地址、邀请码和传统任务白名单，请它挂载。
挂载前会显示人类确认请求，明确授予父节点当前项目整个后代子树的管理权，确认后才开始连接。子节点主动建立 SSE，不需要公网入站端口。

之后在父节点说“发现已挂载 Agent，按角色建立 Leader 模式的群”，或指定 讨论 / Relay 接力模式。信息不足时 AdminAgent 应询问，而不是猜测成员。
Web 群聊列表会刷新；CLI 用 /chat 群UUID 进入，普通消息派发给业务 Agent，/admin 返回管理会话。

进入群聊后，Web 和 CLI 均支持 `/agents`、`/add-agent PATH [角色]`、`/remove-agent PATH`、`/agent PATH role 角色`、`/agent PATH leader`、`/group mode relay|a2a|pmo`。更改对下一轮执行生效，当前执行保留原策略。`/agent PATH provider codex|claude|crabot|mock` 仅配置已停止的本地 Agent，随后 `/agent PATH start` 启动；停止仍需人类确认。Web 的「管理 / 项目」侧边栏分别提供工具和 Skills 管理入口，可查看工具及参数，不包含远端或供应商 CLI 的私有工具。

## 部署与安全

监听地址通过 `BIND_ADDR` 配置；远程 Web 管理还必须配置 `WEB_ACCESS_TOKEN`，浏览器登录用户名为 `crabot`。具体步骤见 [远程 Web 访问](advanced-startup.md#从其它机器打开-web)。公网入口应使用 HTTPS 和受控反向代理。不要分享访问口令、模型密钥或整个数据目录。
AdminAgent 与业务 Agent 都运行在当前节点进程；停止 Web 不影响它们。关闭进程会停止运行，历史保留。
启动时配置不可由 AI 修改；权限请求由用户确认，AdminAgent 不能自己批准。交互保存的模型配置优先于 `.agent.env` 等文件中的默认值；显式导出的环境变量仍优先，不会被交互配置静默覆盖。

Crabot 直接运行本机进程，不需要额外系统沙箱组件。执行命令和 Python 使用当前系统账号的权限，操作确认仍保留。
参见 [管理架构](admin-agent.md)、[Skill 与本机执行](skills-and-sandbox.md)。

多实例使用 `./crabot --name dev --web-port 8787`，另一个实例改用 `--name review --web-port 8788`。数据目录为工作目录下 `.crabot_<name>`；同名同目录禁止重复启动。端口 0 自动分配，端口冲突不会抢占已有服务。权限面板支持「确认/拒绝」及方向键选择，Esc 收起、Ctrl+P 打开。

## 端口、工作目录与 Web 路由

`./crabot --name dev --web-port 8787 --workdir /absolute/project` 启动交互入口，然后对 AdminAgent 说“启动 Web”。端口优先使用显式启动配置，其次使用实例保存的上次成功监听地址，默认 8787；只有明确指定 0 才随机分配。冲突会报错，不自动换端口。每个实例独立保存，不自动启动 Web。

工作目录默认是调用命令时的当前目录，可用 --workdir 或 AGENT_WORKDIR 指定；没有 AGENT_WORKSPACE 配置。命令参数优先。

自带 Web 与后端同源，无需 WEB_CONFIG_ORIGINS；它只用于显式允许独立前端来源，例如 http://localhost:5173。默认不开放额外开发来源，也不支持 *。

Web 使用 Vue Router Hash 路由：`#/projects/GROUP_ID`、`#/management`（旧地址仍兼容）。配置面板以 panel 查询参数表示，刷新和前进/后退保留位置；无有效路由时默认进入项目。Token 不进入 URL。聊天按浏览器本地日期分隔，消息显示时分，悬停查看完整时间。


### 项目与主题
Web 中一个项目就是一个协作群：项目 ID 沿用原群 ID，项目配置维护成员与协作策略；管理 Tab 属于当前节点。左侧不再展示旧上层项目选择器，所有旧命名空间里的协作群汇总为项目列表。
旧存储命名空间和 API 路径暂时保留作为兼容层，旧版聊天 JSONL 不再读写（聊天记录只存知识索引）。新项目级能力绑定以群 ID 为键；没有显式覆盖时继承原命名空间的旧绑定，避免升级时丢失配置。远端节点能力仍由所属节点配置。
能力 API 在原路径增加可选 `?group=GROUP_ID`，工具测试 POST 增加可选 `group` 字段，均校验该群属于请求命名空间。定义仍全局维护，执行与试运行使用相同的项目绑定规则。
左上角使用纯文字 Crabot，不含 Logo 或品牌下拉菜单。太阳／月亮按钮切换日间／夜间模式，选择保存在当前浏览器；刷新前预先应用，减少主题闪烁。


### Agent 管理、简单聊天与远程 Crabot
在管理 Tab 打开「Agent 管理」。本地 Agent 支持保存、编辑（先停止）、启动和人工确认停止，运行器可选 Crabot / Codex / Claude / Mock。配置和运行状态存于当前实例；Crabot 沿用实例模型环境，供应商 CLI 沿用本机 CLI 配置。
项目成员只能从目录选择：已配置的本地 Agent、虚拟 Agent、在线的远程 Crabot。服务端同时校验，不能通过任意路径添加陌生成员。旧项目已有的后代路径仍保留兼容。
简单聊天 `mode: chat` 必须且只能有一个 Agent；该成员也可以是虚拟 Agent。项目名称可留空，首次消息的前 32 个字符会成为标题（合并空白，不额外调用模型）。之后发消息不会覆盖标题，仍可手动重命名。
虚拟 Agent 使用与项目相同的协作策略编辑器，将内部成员组合成一个可复用的能力入口。对调用方仅返回最终结果，不转发内部执行进度。禁止循环引用，最多 8 层；纯本地组合可中断，包含远端执行的中断仍受已有分布式协议限制。
远程连接步骤：
1. 上游在 Agent 管理生成一次性邀请码，默认 24 小时有效。
2. 子节点设置「对外提供的 Agent」（单个或虚拟 Agent），填入上游 URL 与邀请码，申请连接并明确确认子树管理授权。
3. 子节点主动 SSE 连接上游；上游项目选择器出现该远程 Crabot，可选中开始聊天。不要求子节点暴露公网端口。
公网部署需上游可达，使用 HTTPS 反向代理或可信 VPN；非回环监听必须配置管理口令。AK/SK 自动注册后保存在节点的私有数据目录，列表不返回凭据。
对外策略是新加入上游项目时复制的默认值；修改默认值不会覆盖已创建的上游策略副本。挂载 Agent 勾选列表仅限制传统任务路由，不限制已明确授权的子树管理；为空时只提供 A2A 策略调用。
HTTP 管理接口沿用管理鉴权：`GET/PUT /v1/repl/{namespace}/agents`、`POST .../agents/{id}/start|stop`、`POST/PUT .../virtual-agents`、`POST .../peers/invitations|mount`、`GET/PUT .../peers/exposure`。stop/mount 返回待审批请求，不立即执行。旧 namespace 是存储兼容标识，不是 Web 项目。

管理 Tab 直接列出 Agent，点击后在右侧内嵌维护配置。简单聊天成员上限为 1；从多成员模式切换时先选择要保留的成员，再确认切换。虚拟 Agent 的内部策略仅支持接力 / 讨论 / Leader。消息发送先即时回显，日志与列表刷新在后台完成；发送失败保留输入，已接受的消息不会因元数据刷新失败而恢复成待发送文本。
