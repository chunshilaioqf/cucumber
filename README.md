# 黄瓜 · cucumber

**跨平台本地进程与服务统一管理工具**——GUI、本地 HTTP/WebSocket API 与 MCP 接口三轨对等，AI Agent 与外部脚本可直接操作。

## 功能一览

| 模块 | 能力 |
|---|---|
| 仪表盘 | 圆环仪表：CPU/内存/磁盘/网络/GPU（macOS ioreg）实时（默认 1s）分色卡片 + 统计 |
| 进程管理 | 配置命令模板（启动/停止/检查/装依赖）、长驻子进程 PID 跟踪、点火即走模式、系统观察模式（按进程名） |
| Web 服务 | 地址列表、一键复制 URL、调起系统浏览器 |
| 日志查看 | 多日志源（文件/命令）、四种行格式解析（json_lines/logfmt/bracket/plain）、级别/标签/关键词过滤、分页、级别着色与关键词高亮、跟随模式 |
| 对外接口 | HTTP API + WebSocket（axum，仅 127.0.0.1，本地令牌认证）+ MCP（stdio 与 HTTP 双传输） |
| Skill | 内置 `cucumber-manager` 一键安装（目录 + SKILL.md，锁清单防重复，拒绝可执行脚本） |
| 帮助 | 版本/环境信息、仓库与作者、接口快速诊断、常见问题 |
| 设置 | 主题（浅/深/跟随系统）、字体（含系统字体选择器·预览）、字号缩放、背景色（含透明度）、托盘开关、九语言切换 |
| 托盘 | 可选系统托盘（Linux ksni / Win·Mac tray-icon）：实时统计，左键唤起 |
| 更新 | 启动 1 分钟后检查；**下载并安装**（macOS 自动替换+清隔离+重启 / Windows 自动替换重启）或查看网页下载 / 跳过版本 |

## 下载安装

到 [Releases](https://github.com/chunshilaioqf/cucumber/releases/latest) 按系统与芯片选择安装包：

| 系统 | 芯片 | 安装包 | 安装方式 |
|---|---|---|---|
| macOS 12+ | Apple Silicon（M1/M2/M3/M4） | `cucumber-x.y.z-macos-universal.dmg` | 双击挂载 → 把「黄瓜」拖入 Applications → 启动台打开 |
| macOS 12+ | Intel | `cucumber-x.y.z-macos-universal.dmg`（同包双架构） | 同上 |
| Windows 10+ | x64（主流 PC） | `cucumber-x.y.z-windows-x86_64.zip` | 解压 → 双击 `cucumber.exe`（含图标） |
| Windows 10+ | ARM64（Surface 等） | `cucumber-x.y.z-windows-arm64.zip` | 同上 |
| Linux | x86_64 | `cucumber-x.y.z-linux-x86_64.tar.gz` | `tar xf` 后运行 `./cucumber` |
| Linux | ARM64 | `cucumber-x.y.z-linux-aarch64.tar.gz` | 同上 |

> macOS 首次运行若被 Gatekeeper 拦截：**系统设置 → 隐私与安全性 → 安全性一栏 →「仍要打开」**
> （macOS 13- 为右键 →「打开」），或终端执行
> `xattr -dr com.apple.quarantine /Applications/Cucumber.app`。DMG 内附「首次运行说明.txt」。
> 彻底消除提示需要 Apple Developer 账号（$99/年）做签名公证，欢迎 PR 接入。
> 所有包附带 `SHA256SUMS` 可校验完整性。

## 快速开始

```bash
# 构建（Linux 需 X11 开发库：libx11-dev libxcb1-dev libxkbcommon-dev 等）
cargo build --release
./target/release/cucumber          # 启动 GUI
./target/release/cucumber --mcp    # MCP stdio 模式（供 AI 宿主拉起）
./target/release/cucumber --smoke-test 8   # 冒烟自检
```

配置文件：`~/.config/cucumber/config.toml`（macOS `~/Library/Application Support/cucumber/`，Windows `%APPDATA%\cucumber\`），首次启动自动生成（含本地令牌）。

## AI 接入（一分钟）

```bash
TOKEN=$(grep auth_token ~/.config/cucumber/config.toml | cut -d'"' -f2)
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:<端口>/api/processes
```

或在 MCP 宿主配置中添加 stdio 命令 `cucumber --mcp`。完整说明见 [docs/ai-guide.md](docs/ai-guide.md) 与应用内「使用说明」页。

## 平台支持

| 平台 | 状态 |
|---|---|
| Windows 10+ | Win32 + DirectWrite |
| macOS 12+ | Cocoa + CoreText |
| Linux X11 | x11rb（默认） |
| Linux Wayland | 原生后端：`WINDUI_BACKEND=wayland`（默认经 XWayland） |

## 开发

```bash
cargo test                    # 40 项单元/集成测试（core/api/platform/ui 解析）
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
xvfb-run -a ./target/release/cucumber --smoke-test 8   # Linux GUI 冒烟
```

- 架构与决策：[docs/架构设计.md](docs/架构设计.md)
- 需求基线：[docs/需求分析-v1.1.md](docs/需求分析-v1.1.md)
- 更新日志：[CHANGELOG.md](CHANGELOG.md)

## 许可证

MIT © cucumber contributors
