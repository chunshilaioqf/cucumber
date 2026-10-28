# 更新日志

本项目的所有显著变更都记录在本文件中。
格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.2.0] - 2026-10-28

### 新增

- **分发包全面升级**（回应"按操作系统 + CPU 架构分组、开箱即 GUI"）：
  - macOS：发布 **DMG 安装包**（`Cucumber.app`：Info.plist + AppIcon.icns + universal 二进制，双击挂载、拖入 Applications 即可启动图形界面；同包覆盖 Apple Silicon 与 Intel）。
  - Windows：新增 **ARM64**（aarch64-msvc）架构包；exe 资源**内嵌应用图标**（资源管理器/任务栏显示）。
  - Linux：新增 **ARM64**（aarch64-gnu，原生 arm64 runner 构建）架构包。
  - 下载指引：Release 说明与 README 均提供**按系统/芯片分组的下载表**与安装/解封步骤；`SHA256SUMS` 覆盖全部包。
- **应用图标**（三平台）：`tools/gen_icon.py` 纯 Python 确定性生成（PNG/ICO/ICNS/RGBA），窗口与 Dock 显示黄瓜图标；生成器入库可复现。

### 变更

- release.yml 构建矩阵扩展为 6 目标（win-x64/win-arm64/linux-x64/linux-arm64/macos-x64/macos-arm64 → lipo → DMG）；macOS 中间产物不再直接发布 tar.gz。
- 版本号 0.1.0 → 0.2.0。

## [0.1.0] - 2026-10-28

首个可用版本：阶段一（核心闭环）+ 阶段二（日志查看）+ 阶段三（服务与对外接口）+ 阶段四（文档与 CI）全部落地。

### 新增

- **仪表盘**：进程/服务统计、CPU/内存指标、接口监听状态；采集线程 + `App::channel` 回投，空闲零 CPU（FR-DASH-01~03，NFR-PERF-01）。
- **进程管理**：
  - 三种类型（普通进程 / Web 服务 / 其它），Web 服务地址列表（FR-PROC-01~03）。
  - 四类命令模板（启动/停止/检查/装依赖）+ 工作目录 + 环境变量（FR-PROC-04/05）。
  - 启动命令双语义：spawn 长驻子进程（默认）与「点火即走」（ADR-03）；停止三级降级：stop_command → SIGTERM+5s 宽限 → 强杀（ADR-04，R-05）。
  - 系统观察模式：按进程名子串观察 PID/状态/CPU/内存，不干预生命周期（FR-PROC-07/08）。
  - 列表 CRUD、删除二次确认、运行中删除警示（FR-PROC-10~12）。
- **日志查看**：
  - 多日志源（文件 / 读取命令，命令输出限量 5 万行，ADR-05）。
  - 四种行格式：json_lines / logfmt / bracket / plain（含级别嗅探），手写解析器（ADR-02）。
  - 级别多选 + 标签 + 消息体子串过滤（AND），时间过滤在 API 层可用。
  - 文件行偏移索引按页 seek 读取，10 万行级别不整读内存（NFR-PERF-02）。
  - 级别着色 + 关键词黄底高亮（RichDoc span，数据层预计算区间，R-02）。
  - 分页（50–1000 行/页）与跟随模式（FR-LOG-01~07）。
- **所有服务页**：运行中 Web 服务集中列表、一键复制 URL、调起系统浏览器（FR-SERV-01~03）。
- **对外接口**（FR-API-01~04）：
  - HTTP API（axum）：进程 CRUD、操作、仪表盘、服务、日志查询、skill 安装；`GET /api/health` 免认证。
  - WebSocket：每刷新周期推送 dashboard 快照 + 控制消息（start/stop/check/dashboard）。
  - MCP：自实现 JSON-RPC 2.0 子集（ADR-01），stdio 与 HTTP 双传输，9 个工具与 GUI 能力对等。
  - 仅绑定 127.0.0.1，本地令牌（Bearer/query）强制认证，令牌 UI 脱敏展示（NFR-SEC-01）。
  - 端口由系统分配后回写配置持久化。
- **Skill**：内置 `cucumber-manager`（目录 + SKILL.md），frontmatter 校验、拒绝可执行脚本/子目录、`.skill-lock.json` 锁清单、幂等重装（FR-SKILL-01~03，NFR-SEC-03）。
- **使用说明 / 帮助中心**：Markdown 内嵌二进制（include_str!）+ 轻量渲染；版本/构建信息（build.rs 注入）、渲染后端环境信息、接口快速诊断、常见问题（FR-DOC-01/02，FR-HELP-01/02）。
- **CLI**：`--version`、`--mcp`（stdio 服务）、`--smoke-test N`（xvfb 冒烟）。
- **CI/CD**：`ci.yml`（三平台 lint + 测试 + release 编译 + Linux Xvfb 冒烟）与 `release.yml`（tag 触发、原生 runner 矩阵、macOS universal 二进制、tag/Cargo.toml 版本一致性校验、SHA256SUMS）。

### 工程约束

- 平台差异全部收口于 `platform/`（浏览器调起、命令执行、停止信号），UI 层平台无关（NFR-PLAT-02）。
- UI 适配层 `ui/kit.rs` 收口 windui 控件构建调用（NFR-MAINT-03，ADR-07）。
- 命令以参数数组执行、不穿 shell（NFR-SEC-02）；令牌不出现在日志（NFR-SEC-01）。

### 已知限制

- 系统级服务管理为「观察 + 尽力而为信号」（R-03）；systemctl/launchctl/SCM 全量管理请配置命令。
- 日志编码 v1 仅 UTF-8；跟随模式为周期全量重查（尾部增量索引留待后续）。
- Linux 系统托盘不可用（windui 平台限制）；Wayland 原生后端下全局热键不可用（协议限制）。
