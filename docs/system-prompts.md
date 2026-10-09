# 系统提示词

默认模板统一使用英文，维护在仓库 `conf/` 中并随版本内嵌在程序中。回答仍遵循用户的语言。启动 `crabot` 时先读取和校验提示词，**不复制默认模板**。

## 默认与实例覆盖

读取顺序：**实例覆盖文件 → 当前版本内置默认模板**。按文件独立覆盖，没有覆盖的模板会随升级自动更新。

覆盖文件放在：

- 默认实例：`~/.crabot/conf/`
- `crabot --name work`：`~/.crabot_work/conf/`
- 指定 `--data-dir`：该目录下的 `conf/`

首次启动不创建提示词文件；Web/CLI 保存修改时才创建对应覆盖文件和目录。也可以自行创建目录及同名 UTF-8 Markdown 文件。已有覆盖不会在启动或升级时被改写，即便其内容恰好等于当前默认模板，仍视为显式覆盖。

如果之前使用过复制模板的版本，其已有文件也会被视为覆盖。不会猜测并删除用户文件；可逐项恢复默认，让它们重新跟随版本更新。

## 修改与恢复

Web：**管理 → 系统提示词**，每项显示“随版本默认”或“实例覆盖”。进入编辑，将开关切到“实例覆盖”后修改并保存。“恢复随版本默认”并保存会删除对应覆盖文件，而不是把默认文本写入文件。

CLI：

```text
/prompts
/prompts show discussion
/prompts set response Lead with the conclusion and keep answers concise. Respond in the user's language.
/prompts import discussion /path/to/discussion.md
/prompts reset discussion
```

`set` 用于一行内容，`import` 用于多行文本；SSH 下使用远端文件路径。`reset` 移除指定的实例覆盖，恢复当前版本默认。也可直接用编辑器修改或删除实例覆盖文件。

保存后无需重启，后续任务读取新内容。不会重写已发给模型的请求。正在进行的讨论在后续成员派发或职责复核时会读取对应的新规则。保存时检查原内容，避免静默覆盖其他页面或编辑器已经保存的修改。

## 文件及范围

| 文件 | 用途 |
| --- | --- |
| `agent.md` | Crabot 内置 Harness 的系统指令 |
| `response.md` | 各运行器共享的默认回复要求 |
| `management.md` | 管理会话行为 |
| `discussion.md` | 讨论、分工和共识原则 |
| `participation.md` | 普通成员的职责判断 |
| `addressed.md` | 明确指派后的回应要求 |
| `participation-recheck.md` | 申请让出的复核要求 |
| `leader.md` | Leader 分配任务的原则 |
| `relay.md` | 接力模式协商优先级的原则 |

覆盖只影响当前 Crabot 实例，不修改其他实例或远端节点的文件；但本实例派发的任务会携带相应协作要求。Codex、Claude 等保留自己的原生系统指令，Crabot 的管理、协作和回复要求作为任务指令传给它们。

显式配置的 `MODEL_SYSTEM_PROMPT` 优先于 `agent.md`；Agent 自定义回复要求优先于 `response.md`。提示词不是权限配置，不能取消命令审批、明确 `@` 指派后禁止让出的校验或有限重试。控制信号保留协议原文，JSON 输出格式由程序追加，避免修改规则后破坏调度协议。

覆盖文件不能为空，单文件最大 128 KiB，不能使用符号链接。读取无效文件会报告具体路径，不会悄悄忽略错误。

## 接口

Web 与 CLI 共用同一配置服务，HTTP 仅通过当前实例的本机管理入口提供：

- `GET /v1/config/prompts`：目录和提示词列表，每项包含 `id`、`title`、`description`、`path`、`content`、`default`、`overridden`；`path` 是可选覆盖文件的位置，不表示文件已经存在。
- `PUT /v1/config/prompts`：保存覆盖使用 `{"id":"discussion","content":"New instructions","expected":"编辑前的完整内容"}`。
- 同一 PUT 接口传 `{"id":"discussion","reset":true,"expected":"编辑前的完整内容"}` 移除覆盖。

只有预定义文件可编辑，接口不接受任意路径。保存采用同目录临时文件替换。发布包保留 `conf/` 方便查看源模板，但运行不依赖当前工作目录。
