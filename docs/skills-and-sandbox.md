# Skill 与本机执行

管理能力与项目能力共用 Tool / Skill 框架，分别加载自己的能力目录。工具通过宏注册，Skill 通过目录加载。

读取 Skill 不代表授权运行代码。Python、浏览器和文件操作统一经 shell 执行，遵循命令审批与白名单。

## 启动策略

```bash
export CRABOT_EXECUTION_PROFILES_JSON='[{"id":"default","network":"host","timeout_seconds":30}]'
export CRABOT_EXECUTION_PROFILE=default
export CRABOT_EXECUTION_ALLOWED_PROFILES=default
crabot --workdir /absolute/project --outside-access ask
```

原 CRABOT_SANDBOX_* 配置名保留为兼容别名，新配置优先。它们只配置执行策略，不再启用系统沙箱。
Python 由 shell 命令选择解释器（如 python3 或项目虚拟环境）。Profile 支持 id、network、timeout_seconds、secret_env。
network 只能是 host；旧 none 配置会明确报错，不会假装仍能隔离网络。
timeout_seconds 为 1..120，限制单次工具执行而不是整个对话。最多四个执行任务并发，读取输出不设 64 KiB 上限；保留 CPU 时间与文件描述符数量限制，大输出建议写文件或分段查看。

## 执行边界

Crabot 不再使用 sandbox-exec 或 bubblewrap。Shell、Python 和 Skill 在指定工作目录中直接运行，具有当前系统账号的文件和网络权限，包括工作目录外文件。工作目录不是隔离边界。

操作确认、命令白名单仍然有效。拒绝确认不会启动命令；允许执行不代表程序内部的每次文件访问都会再询问。文件工具自己的目录外访问策略（deny / ask）仍保留，但无法约束 Shell 或 Python 内部的系统调用。

执行器继续使用临时 HOME/TMPDIR 和经过筛选的环境变量，避免自动继承无关密钥；这不是文件访问隔离。Skill 的执行副本保留在工作目录 tmp，源文件不变。上传附件的执行副本位于工作目录 .crabot/tmp/（命名实例为 .crabot_<别名>/tmp/），原件保留在实例数据目录。

Codex / Claude 不再套 Crabot 的系统沙箱。保留现有 Provider 权限模式映射：ask/auto 使用 Codex read-only、Claude plan；只有用户明确选择 full 时才使用原有更宽松模式。登录凭据导入仍经过文件访问授权，未强制关闭厂商自带安全机制。Chromium 自身沙箱保留。

## 生命周期与验证

execution 模块的进程内 supervisor 管理并发、超时、调用者取消和关闭。RAII 清理进程组及暂存目录，取消任务会终止同进程组的子进程；主动脱离进程组的程序不在此保证内。

测试覆盖本机 Shell/Python 的网络和目录外文件访问、任务取消后的子进程清理，以及审批与 Provider 协议。真实厂商 CLI 的所有版本和登录方式未逐一验证。
