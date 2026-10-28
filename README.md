# 黄瓜 · cucumber

**跨平台本地进程与服务统一管理工具**——GUI、本地 HTTP/WebSocket API 与 MCP 接口三轨对等，AI Agent 与外部脚本可直接操作。

## 功能一览

| 模块 | 能力 |
|---|---|
| 仪表盘 | 进程/服务统计、CPU/内存指标、接口监听状态（Signal 定向刷新，空闲零 CPU） |
| 进程管理 | 配置命令模板（启动/停止/检查/装依赖）、长驻子进程 PID 跟踪、点火即走模式、系统观察模式（按进程名） |
| Web 服务 | 地址列表、一键复制 URL、调起系统浏览器 |
| 日志查看 | 多日志源（文件/命令）、四种行格式解析（json_lines/logfmt/bracket/plain）、级别/标签/关键词过滤、分页、级别着色与关键词高亮、跟随模式 |
| 对外接口 | HTTP API + WebSocket（axum，仅 127.0.0.1，本地令牌认证）+ MCP（stdio 与 HTTP 双传输） |
| Skill | 内置 `cucumber-manager` 一键安装（目录 + SKILL.md，锁清单防重复，拒绝可执行脚本） |
| 帮助 | 版本/环境信息、接口快速诊断、常见问题 |

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
