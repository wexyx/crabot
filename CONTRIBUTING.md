# 参与开发

项目介绍、安装与核心架构见 [README](README.md)。

## 源码结构

```text
crates/
  agent-node/         启动装配、CLI、HTTP、Core、A2A、存储
  agent-runtime/      Provider、Harness、工具、Skill、执行权限
  agent-protocol/     消息与事件协议
  agent-tool-macros/  工具注册宏
apps/web/             Vue Web 入口
conf/                 随版本发布的英文提示词模板；实例 conf/ 仅保存用户覆盖
skills/system/        随版本发布的内置 Skill
scripts/              测试、启动器与发布打包
```

Rust 的 `mod.rs` / `lib.rs` 只负责模块声明和导出；接口、实现、状态和厂商解析放在各自职责模块中。

系统提示词集中维护在 `conf/`，使用及覆盖规则见 [系统提示词](docs/system-prompts.md)。新增模板时同时登记 `agent-runtime/src/prompts/catalog.rs`，不要把实例的修改写回源模板。


仅源码开发需要 Rust（支持 edition 2024）、Node.js 20+ 和 pnpm：

```bash
git clone https://github.com/wexyx/crabot.git
cd crabot
pnpm install
pnpm web:build
cargo build -p agent-node
./crabot
```

源码运行使用 `./crabot`；普通用户安装后的命令始终是 `crabot`。

```bash
# 测试
cargo test --workspace
node --test apps/web/src/*.test.js scripts/*.test.mjs

# 本地发行包示例（先构建 Web）
export CRABOT_RELEASE_VERSION=v1.2.3
cargo build --release --locked -p agent-node
bash scripts/package-release.sh aarch64-apple-darwin
```

推送版本标签会触发 `.github/workflows/release.yml`，构建各平台程序、Web 和 Skill，生成安装包及 SHA-256，并创建 Draft Release。维护者验证后正式发布，安装器才会将其作为可安装版本。SHA-256 是完整性校验，不等于代码签名；当前没有实现 macOS 公证或包签名。

Release 构建将 tag 写入二进制，启动页和 `crabot --version` 显示同一版本。打包时校验版本与 tag 一致；已有安装包不会自动改变。未设置 `CRABOT_RELEASE_VERSION` 的本地构建显示 `v<Cargo 包版本>-dev`，不伪装成发行版。

升级时重新运行安装脚本，安装器切换命令到新版本并保留旧版本目录和实例数据。详细安装配置见 [进阶安装与启动](docs/advanced-startup.md)。

## 提交变更

修改前明确模块职责，保持 CLI 与 Web 使用同一应用能力。新增 Provider 应实现统一合约，不在调用方增加厂商专用分支；新增工具使用统一注册机制。新增内置 Skill 放入 `skills/system/`，不要把用户实例数据或凭据提交到仓库。

提交前运行与变更相关的测试；涉及 Web 时运行 `pnpm web:build`，涉及发布时验证安装包包含程序、Web、Skill、文档及许可证。提交说明中列出改动和验证结果，不把单一平台测试当作跨平台验证。

项目采用 [MIT License](LICENSE)。
