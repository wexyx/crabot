# 知识库、任务与引导消息

## 知识文档

Web 项目侧的工具、Skills、知识库在同一组页签；管理页面不展示知识库入口。知识库支持搜索、网址导入、上传、新建、编辑和删除。文档保存到当前实例统一的图数据库（`<实例目录>/knowledge/documents/graph.db`）；管理、所有项目和本地 Agent 共享同一知识库，不受聊天选择影响。不同 Crabot 实例仍彼此隔离。

Web 默认展示全部文档，按导入时间倒序分页（默认每页 20 篇，可切换 10/20/50/100），清空搜索即恢复全部列表。编辑文档不会改变导入时间与排序。搜索接口返回 `total`，`created_at` 为 Unix 毫秒时间戳；缺少原始导入时间的已有记录显示 `—`，不会用修改时间冒充。

Agent 可通过 `find(target="doc", query="关键词")` 搜索，传入 `id` 分块读取。结果的 `next_offset` 用于继续读取。文档块可用文档 ID、版本、块序号引用。用 `find(target="tool", name="doc")` 发现维护工具，再调用 `doc`：

```json
{"action":"import","url":"https://example.com/docs","depth":1,"max_pages":20}
```

也可传 `path` 导入本地文档；目录外访问遵循当前工作目录授权。`save` 接收 `title`、`content` 和可选 `source`；更新、删除必须带 `id` 和读取时得到的 `expected_version`，避免覆盖他人的修改。同一实例内相同来源不会重复创建。

支持网页、文本型 PDF、Word DOCX、UTF-8 Markdown/纯文本。单文件最多 20 MiB，提取后正文最多 4 MiB。扫描件需先 OCR；旧 Word `.doc` 需转为 `.docx`。动态网页、登录页面和 `#/...` 路由页面可由浏览器 Skill 提取后用 `doc save` 入库。不会自动绕过登录、验证码或执行网页中的脚本。

网址导入深度范围 0–5，0 仅导入当前页；页面上限 1–100，默认 20。只跟随同源链接；逐页返回成功/失败情况。已成功入库的页面不会因为后续失败而回滚。自动剔除 `utm_*`、`gclid`、`fbclid` 等跟踪参数；文档标识和未知 query 参数保留。`ignored_query_params` 可额外指定要删除的参数。普通 fragment 去掉，前端路由 fragment 不会误当同页合并。仅允许公网 HTTP(S) 的 80/443 端口，检查 DNS 与每次重定向，禁用继承代理并固定验证后的解析地址。

CLI：

```text
/docs search 架构
/docs import https://example.com/docs
/docs import {"url":"https://example.com/docs","depth":2,"max_pages":30,"ignored_query_params":["ref"]}
/docs import /path/to/document.pdf
/docs read DOCUMENT_ID
/docs save {"title":"设计","content":"正文"}
/docs save {"id":"DOCUMENT_ID","expected_version":1,"title":"新标题","content":"新正文"}
/docs delete {"id":"DOCUMENT_ID","expected_version":2}
```

HTTP 与 CLI/Agent 共用服务层：`POST /v1/documents` 接收 `action=search|read|save|import|delete`；上传接口为同路径 `/upload?name=...`，请求体是文件字节。

## 运行中的任务

`shell` 默认启动进程并等待最多一秒，尚未退出时返回 `session_id`。后续可 `action=read` 查询增量输出、`write` 发送输入、`stop` 停止。`interactive=true` 使用 PTY，适合登录等交互程序；进程可跨模型回合存活。不同项目、聊天、Agent 的会话互相隔离，Agent 的凭据 HOME 按项目与 Agent 独立保留在实例目录的 `runtime/shell` 下。

Web 聊天右上角的任务图标打开会话输出与输入抽屉。CLI 用 `/process`、`/process read ID`、`/process input ID`、`/process stop ID`。私密输入不会进入聊天历史；发送后该进程的后续输出只对人可见，不交给模型。模型向既有进程写入内容仍需正常审批，不能靠白名单命令绕过输入审批。

输出缓冲有上限；空闲 30 分钟的进程会回收。重置上下文、中断、删除聊天、运行失败和正常退出会清理相关进程。主动脱离会话的系统守护进程不属于托管会话。

## 引导消息

任务运行时仍能输入消息。Web 显示“发送引导”，CLI 直接继续输入；中断按钮/快捷键保留。

引导先落盘再交给运行器。Crabot 在下一次模型调用或工具边界读取新要求，并跳过尚未开始的旧工具调用；已经开始的工具不会被强杀。CLI Provider 与远端 Agent 在当前调用结束后接收后续引导，不宣称支持它们没有提供的进程内热注入。只有被接收的引导才返回成功；任务恰好结束时提示重新发送。

## 默认 Skills 与压缩

`web-search` 优先 Google，必要时切换百度等搜索引擎，通过已有浏览器 Skill 获取原始来源。`coding` 要求遵循仓库规则、语言风格与清晰职责，验证受影响模块的覆盖率，目标不低于 90%；缺少 AST/测试工具时走普通安装审批。实际覆盖率需要测试测量，不会因为启用 Skill 就自动达到目标。

摘要超长时会要求模型精简，最多追加两次重试；仍超长但结构有效时缩短生成的摘要并标记信息省略，保留当前要求和近期原文。原始历史不删除；无效摘要不会覆盖上下文。后续的用户引导也按原文保护。
