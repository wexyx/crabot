pub(super) const TEXT: &str = r##"Crabot · 命令
直接输入消息开始对话；输入 / 打开命令菜单。

入口
  /manage                  进入管理（兼容 /admin）
  /back                    返回上一个聊天；管理页空闲且输入为空时 Ctrl+C 同效
  /chat                    选择新建聊天或进入已有聊天（同 /sessions）
  /new                     重置当前上下文，保留聊天记录
  /agents · /members       Agent 目录 / 当前项目成员
  /agent-config            默认 Agent 配置
  /prompts                 当前实例系统提示词；查看、修改或导入文本
  /connections             查看上游和下游连接
  /disconnect upstream|downstream NAME  断开连接（/reconnect 恢复）
  /connect URL             申请连接上游，需人工确认
  /server [status]          Server 状态
  /history                 恢复当前聊天；/tools [序号] 查看调用
  /attach 文件路径         下一条消息附带文件或图片；SSH 下使用远端路径
  /detach                  清空待发送附件，不删除原文件
  /update                  下载最新版本，完成后重启生效
  /interrupt · /exit        打断 / 退出

更多操作
  /help commands           全部命令及说明
  /help agents             新增、编辑、删除、测试 Agent
  /help projects           新建项目、配置模式与成员
  /help network            组网、服务与权限
  /help capabilities       Tool / Skill 维护、绑定和测试
  /help chat               会话导航与快捷键
"##;
pub(super) fn render(name: &str) -> Result<String, String> {
    if name == "commands" {
        return Ok(super::command_catalog::ENTRIES
            .iter()
            .map(|entry| format!("  {:<18} {}", entry.name, entry.description))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    topic(name).map(str::to_owned)
}
pub(super) fn topic(name: &str) -> Result<&'static str, String> {
    Ok(match name {
        "" => TEXT,
        "agents" => {
            r##"Agents（本地可维护，远端只读）
  /agent list                         完整目录，不受当前会话影响
  /agent show ID                      详情与版本
  /agent add ID PROVIDER 角色          新增（crabot/codex/claude/opencode/mock）
  /agent save JSON                    新增或编辑完整配置
  /agent virtual JSON                 新增或编辑虚拟 Agent
  /agent start ID                     启动
  /agent stop ID                      申请停止，/approve ID 确认
  /agent delete ID VERSION --confirm  删除配置，保留记录；引用中不可删除
  /agent test PATH                    进入独立测试聊天，恢复历史
  /agent-config                       默认 Agent 配置（兼容 /admin-config）

save 示例：
/agent save {"client_id":"coder","provider":"codex","role":"开发","expected_version":1}
编辑、删除前先停止。virtual 的 JSON 字段为 name、policy、expected_version；更新时加 id。
远端 PATH 使用目录返回的 path（以 / 连接），只能查看和测试，不能编辑。
"##
        }
        "projects" => {
            r##"项目即群组，不是旧存储命名空间
  /project                            列出项目
  /project GROUP_ID                   进入项目（同 /chat）
  /project create JSON                创建；name 可省略
  /project configure ID JSON          保存 name、policy、expected_version
  /members                            当前项目成员；/agents 为完整目录
  /add-agent PATH [角色]               加入成员
  /remove-agent PATH                  移除成员
  /agent PATH role 职责                项目内角色
  /group mode chat|relay|discussion|leader        修改模式
  /group instructions 内容            修改共同要求

创建示例：
/project create {"policy":{"mode":"chat","members":[{"path":["default"],"role":"助手"}],"rounds":1,"leader":null,"instructions":""}}
完整 policy 与 Web 一致：relay_strategy、members 顺序、leader、rounds、instructions。
/namespace 已废弃；项目列表用 /chat，管理对话用 /manage。
"##
        }
        "network" => {
            r##"组网与 Server
  /connections             已挂载上游与状态
  /connect URL             申请连接；不会直接授权
  /approve ID · /deny ID   确认或拒绝管理请求
  /server status           Server 状态
  /server start [PORT]     启动；省略复用端口，0 为随机
  /server stop             只停止 Server，CLI 与 Agent 继续运行
  /allow-path ID           允许目录或命令请求；/deny-path ID 拒绝
  确认 / 拒绝              处理唯一待审批请求，不发送给模型

连接上游会允许发现和调用本节点 Agent，须确认信任；公网使用 HTTPS 或 VPN。
"##
        }
        "capabilities" => {
            r##"Tool / Skill（使用与 Web 相同的数据结构和版本校验）
  /capabilities list JSON          列出库与绑定
  /capabilities save JSON          保存、启停或删除非硬编码定义
  /capabilities bind JSON          全局/项目/Agent/项目内 Agent 绑定
  /skills list|save JSON           Skill 定义与上传文件内容
  /tool-library list|save JSON     Agent 工具配置
  /tool-library test JSON          测试工具（仍需正常权限确认）
  /tool-library test-status JSON   查看测试结果
  /tool-library test-cancel JSON   取消测试

列表示例：
/capabilities list {"scope":"business","kind":"tool"}
绑定和保存：同上增加 body（与 Web 请求体相同）；绑定可指定 agent、group。
scope 为 management 或 business；kind 为 tool 或 skill。
/skills save {"scope":"business","body":{"expected_version":0,"definition":{"id":"guide","description":"说明","enabled":true,"allow_python":false,"files":{"SKILL.md":"# 说明"}}}}
/tool-library test {"scope":"business","agent":"default","body":{"name":"shell","arguments":{"command":"pwd"}}}
/tool-library test-status {"id":"测试ID"}
Skill 运行测试使用同一工具测试通道（find / shell），不跳过命令授权。
"##
        }
        "chat" => {
            r##"聊天与历史
  /manage · /admin         管理
  /chat                    上下选择新建 / 已有聊天，Enter 进入、Esc 取消
  /chat ID或名称           直接进入；支持唯一 ID 前缀
  /back                   返回上一个聊天
  /history [admin|群组ID]  恢复历史
  /resume SESSION_ID      恢复管理会话
  /allowlist [add 命令|remove 命令|reset|clear]  查看/编辑自动批准白名单
  /permissions ID [ask|auto|full] [--confirm-full-access]  查看/切换执行权限
  /new                    当前聊天开启新上下文，保留历史
  /tools [序号]           展开工具调用
  Enter 发送；Alt+Enter 换行；↑/↓、PgUp/PgDn 切换输入历史
  输入 / 筛选命令；菜单内 ↑/↓ 选择、Tab 补全、Enter 确认
  无菜单时 ↑/↓ 浏览输入历史；Esc 打断或清空；Ctrl+P 审批；Ctrl+D 退出
  鼠标使用终端原生滚动、拖选和复制
"##
        }
        _ => {
            return Err(
                "帮助主题：commands / agents / projects / network / capabilities / chat".into(),
            );
        }
    })
}
