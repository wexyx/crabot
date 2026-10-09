# 进阶安装与启动

日常使用直接运行 `crabot`，以下配置均为可选。安装与架构见 [README](../README.md)。

## 系统要求

支持 macOS Apple Silicon / Intel、Linux x86_64 / ARM64。Linux 最低支持 Debian 12（glibc 2.36），不再支持 Debian 10/11。安装包携带 C++ 运行库，OpenSSL 使用系统版本；精简系统先执行 `sudo apt-get update && sudo apt-get install -y ca-certificates libssl3 libatomic1`。较新的 Debian 版本如果替换了包名，由发行版提供对应的 OpenSSL 3 运行库（如 `libssl3t64`）。安装脚本会在替换现有命令前检查缺失依赖并验证启动。其它发行版需具备兼容的 glibc 和 OpenSSL 3；Alpine/musl 与 Windows 暂无对应安装包。

发布流程在 Debian 12 / GCC 13 镜像中编译，使用与 Cargo.lock 对齐的 Ladybug compat 预编译库。下载或链接检查失败会立即停止，不再回退到全量源码编译。产物在干净 Debian 12 容器中验证启动和动态依赖。

Crabot 的本机执行不依赖额外隔离组件。Python、Codex、Claude CLI 按需另行安装；使用对应 CLI 助手前，需要完成其账号认证。

安装器从正式 Release 下载程序、Web 和内置 Skill，验证 SHA-256，再创建命令链接并配置 PATH。没有对应平台的正式安装包时会报错，不会自动转为源码编译。

## 多实例与目录

```bash
# 命名实例：数据在 ~/.crabot_review，使用独立端口
crabot --name review --server-port 8788

# 自定义工作目录
crabot --workdir /path/to/project

# 自定义实例数据目录，不能与 --name 同时使用
crabot --data-dir /path/to/instance

# 允许对目录外访问发起人工确认，不是直接放行
crabot --outside-access ask

# 预编译安装版只启动 Server；需要提前配置好默认 Agent
crabot --headless
```

默认工作目录为用户主目录；实例目录默认为 `~/.crabot`，两者不绑定。目录锁拒绝多个进程同时写同一个实例。旧版目录不会自动搬迁，继续使用已有数据时通过 `--data-dir` 指定。

Server 首次默认端口为 8787，后续复用实例保存的端口。显式传入 `--server-port 0` 才随机分配；CLI 内也可用 `/server start 8788` 指定端口。

## 启动配置

无需提前创建配置文件。需要固定启动参数时，在实例目录维护 `.agent.env`，例如：

```dotenv
AGENT_WORKDIR=/absolute/path/to/project
BIND_ADDR=127.0.0.1:8787
AGENT_OUTSIDE_ACCESS=ask
```

读取顺序与优先级：命令行参数 > 显式环境变量 > 运行目录 `.agent.env` > 实例目录 `.agent.env` > 默认值。相对目录相对于配置文件所在目录解释。文件只接受字面量 `KEY=value`，不执行 Shell 命令或变量替换。

`--name` / `--data-dir` 先决定实例位置，然后读取该实例配置。实例文件不能通过 `CRABOT_DATA_DIR` 重定向自身；运行目录配置可以设置它，但不能覆盖显式命令行或环境变量。

默认 Agent 的交互配置保存为 `default-agent.json`。已保存的模型配置优先于 `.agent.env` 中的模型默认值；显式模型环境变量仍可覆盖。这样旧模板不会让每次启动重新询问模型信息。

## 固定安装版本与位置

CLI 启动后后台检查更新，有新版本时在底部标红提示。在对话输入框执行 `/update` 下载并安装最新正式版本，完成后提醒重启。`crabot --version` 查看当前版本。
安装版更新沿用原安装位置，不终止运行中的实例，不修改配置或聊天记录，重启后生效。下载或校验失败时不切换命令链接。源码运行时 `/update` 安装发行包，不会修改 Git 工作区。
旧版尚未内置 `/update` 命令时，先重新执行一次安装命令。

将示例版本替换为 [Releases](https://github.com/wexyx/crabot/releases) 中实际存在的正式版本：

```bash
curl -fsSL https://raw.githubusercontent.com/wexyx/crabot/main/install.sh | CRABOT_VERSION=v0.1.0 CRABOT_INSTALL_PREFIX="$HOME/.local" bash
```

重复执行 README 中的一行安装命令即更新到最新正式发布版本，更新后重启 Crabot 生效。安装器自动建立命令链接并写入 Shell PATH，升级保留旧版本目录，不修改实例数据。已有 Shell 文件在首次追加配置前备份；重复安装不重复追加相同配置。

zsh 写入 `${ZDOTDIR:-$HOME}/.zshrc`；bash 写入 `.bashrc` 和生效的登录配置；fish 写入配置目录的 `conf.d/crabot.fish`；其它 Shell 写入 `.profile`。自定义 Shell 若不读取这些文件，需要自行接入对应启动文件。

安装器是子进程，不能修改当前父终端环境。新开终端后直接运行 `crabot`；不想新开终端时，可以使用安装器打印的完整命令路径。

## 排查启动问题

- 找不到命令：先新开终端，检查安装输出是否成功配置 PATH；自定义 Shell 检查其启动文件。
- 端口占用：CLI 内使用 `/server start 8788`，打开返回的 Web 地址。
- 实例已运行：使用现有进程，或先退出，再启动；不要删除运行中的锁文件。
- 工作目录与预期不同：检查显式参数、环境变量、两处 `.agent.env` 和项目自身设置。
- 下载失败：确认正式 Release 与本机平台资产存在，安装器不会退回源码编译。
- 旧版 v0.1.4.3 及之前的 Linux 包提示缺少 `libssl.so.3`：这些包仍依赖 OpenSSL 3 和较新的 glibc，旧系统应升级到包含兼容性修复的新版本。不要将 `libssl.so.1.1` 软链接成 `libssl.so.3`，也不要手动替换系统 glibc。

备份与执行权限等高级功能见 [使用与配置](usage.md)。

## 本地存储与执行边界

每个实例使用独立数据目录和进程锁：

| 数据 | 默认位置 / 形式 |
| --- | --- |
| 实例目录 | `~/.crabot/` |
| 启动配置（可选） | `~/.crabot/.agent.env` |
| 默认 Agent 配置 | `~/.crabot/default-agent.json` |
| 状态变更 | `state.jsonl`，每行追加一次事务 |
| 聊天事件 | `knowledge/<项目>/graph.db`，每项目一个知识索引 |
| 默认工作目录 | 用户主目录 `~` |

状态写入只记录变化，不在每次操作时重写全部状态；启动时回放恢复。聊天记录与运行状态分开保存（聊天索引在 `knowledge/` 下）。备份时先停止实例，再备份完整目录；旧 `state.json` 若作为恢复基线存在，应一并保留。旧版本遗留的 `chats/` JSONL 目录不再读写，可自行清理。

启动配置默认从实例目录读取，运行目录的 `.agent.env` 可覆盖它；命令行参数和显式环境变量优先。交互保存的模型配置优先于环境文件中的模型默认值，避免旧模板覆盖已保存设置。

工作目录不是文件副本，修改直接落在本机。操作确认与白名单控制是否执行，命令和 Python 使用当前系统用户的文件及网络权限。管理 HTTP 接口仅允许本机回环访问，公网 A2A 接入不等同于开放 Web 管理权限。配置中的密钥是本地明文，配置文件使用仅当前用户可读写的权限，不应公开或提交。
