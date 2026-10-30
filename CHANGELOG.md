# 更新日志

本项目的所有显著变更都记录在本文件中。
格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.4.1] - 2026-10-29

### 修复 / 新增

- **Windows 无控制台黑窗（任务 2，ADR-17）**：发布版启用 GUI 子系统（`windows_subsystem = "windows"`）——普通应用般仅一个窗口；`--mcp` 模式自动重附父控制台（`AttachConsole` + CRT `_dup2` 句柄重绑），stdio 行协议不受影响。
- **Windows 安装包（任务 3）**：CI 新增 Inno Setup 步骤，产出 `cucumber-<ver>-windows-<arch>-setup.exe`——安装向导（中/英双语）、开始菜单与桌面快捷方式、可选开机自启、卸载器；便携 zip 同步保留；应用内更新优先选择安装包并静默安装（/VERYSILENT）。
- **Windows/Linux GPU 使用率（任务 4，ADR-16）**：Windows 经 WMI `GPU Engine(*engtype_3D)\Utilization Percentage` 性能计数器汇总（与任务管理器同源，NVIDIA/AMD/Intel 全厂商）；Linux 经 sysfs `gpu_busy_percent`（AMD/Intel）或 `nvidia-smi`（NVIDIA）。开源库取舍：`nvml-wrapper` 等仅 NVIDIA 单厂商，选择零依赖原生数据源方案。
- 57 项测试全过。

## [0.4.0] - 2026-10-29

### 新增 / 修复（七项任务 + 图标一致性）

- **系统托盘跨平台重构（任务 5，ADR-15）**：Linux 改用 `ksni`（DBus StatusNotifierItem，纯 Rust、零 GTK/C 依赖，X11/Wayland 通用），Windows/macOS 用 `tray-icon`。菜单首项实时统计、tooltip 周期刷新、左键唤起、右键退出。默认关，设置页开关，重启生效。
- **真正的应用内更新（任务 6）**：更新对话框新增「下载并安装」——按平台自动选择资产（DMG/zip/tar.gz，下载前校验资产存在），macOS 后台脚本挂载替换 /Applications/Cucumber.app 并执行 `xattr -dr com.apple.quarantine`（用户不再需要手动放行），Windows 等待旧进程退出后替换 exe，两者均自动重启；原按钮改名「查看网页下载」。
- **AI 提示词双版本（任务 4）**：对外接口页拆分「复制 MCP 接入提示」与「复制 Skill 接入提示」；Skill 版点击前检查技能是否已安装，未装则提示先安装。
- **GPU 使用率（任务 3）**：macOS 经 `ioreg` 性能统计尽力解析（Intel `Device Utilization %` / Apple Silicon `IOGPUUtilization`，含十六进制与 0–10000 刻度归一），仪表盘 GPU 卡接真实数据；其他平台 N/A。
- **刷新频率（任务 3）**：默认 3s → **1s**（配置项可调）。
- **字体选择器（任务 2）**：设置页新增「选择系统字体」按钮——枚举系统字体（Linux fc-list / Windows 注册表 / macOS 字体目录）并在对话框中**用每种字体实际预览渲染**，点击即应用。
- **图标资产与 SVG 一致（任务 8）**：`assets/logo.svg` 成为唯一设计真相源：`examples/gen_icon_assets.rs`（resvg）渲染 → `tools/package_icon_assets.py` 派生 icns/ico/窗口与托盘 RGBA，安装后的应用图标即 SVG 图形。
- **列表行右侧间距（任务 7）**：进程/服务/日志行水平 padding 10→14。
- **本地化扩展（任务 1）**：新增顶部菜单栏、编辑表单全部字段、更新对话框按钮、字体选择器、AI 提示按钮等约 40 组文案 ×9 语言。

### 变更

- 依赖新增 `ksni`（Linux）/`tray-icon`（Win/mac）；56 项测试全过（新增 GPU 解析 4 项、updater 2 项、托盘桥接由冒烟覆盖）。

## [0.3.1] - 2026-10-28

### 修复（严重）：外部运行的服务不可见

**症状**：服务实际已启动并运行，管理程序却显示「已停止」，且不出现在「所有服务」页。

**根因**：运行状态的唯一真相源是「本工具 start_command 启动的子进程仍存活」。三类真实场景全部误报：外部启动（systemctl/手动/nohup）；点火即走服务在**工具重启后**仍在运行（状态表清零）；spawn 子进程随工具退出被终止后由外部重新拉起。

**修复（ADR-14）**：服务端口连通性升级为第二真相源，与状态机融合为「有效状态」：
- 新增 `core/port_probe`：对配置了地址的 Web 服务周期 TCP 探测（800ms 超时，任一 tcp/http/https 地址连通即 up；udp 跳过）。
- **发现运行**：端口 up 而状态机非 Running → 显示「运行中（外部）」并挂「端口✓」徽章；进入「所有服务」页与仪表盘运行计数；API 新增 `state_machine` / `port_up` / `externally_running` 字段。
- **不否定运行**：端口 down 不推翻状态机（进程可能存活但端口未就绪），探测只做「发现」。
- 「检查」按钮在未配置 check_command 时退化为端口探测（连通 = 健康）。

**验证**：端到端复现——外部启动 `python3 -m http.server 18099`，黄瓜仅注册配置（从未 start）：state=运行中 / state_machine=已停止 / port_up=true / externally_running=true；服务列表由空变 1；dashboard running=1；check 端口探测 healthy=true。共 49 项单元测试（新增端口探测 3 项 + 状态融合 1 项）。

## [0.3.0] - 2026-10-28

### 新增（十项任务整体落地）

- **仪表盘重做（任务 5）**：圆环仪表（自绘 Widget：弧环 + 中心百分比 + 副文本）展示 CPU / 内存 / 磁盘 / 网络，每类独立半透明色卡，新增磁盘用量与网络上下行速率采集（sysinfo Disks/Networks 差分）；GPU 因 sysinfo 无传感器接口以说明卡占位（帮助页注明）；下方统计卡显示进程/服务/接口状态。
- **设置页（任务 3）**：主题三模式（浅色 / 深色 / 跟随系统，`on_system_theme_changed` 驱动）；字体族输入 + 字号 80%–150% 全局缩放；窗口背景色（RGBA 含透明度，作用于应用内绘制层）；系统托盘开关（重启生效）；**九语言切换**（简体中文/繁體中文/English/日本語/한국어/Русский/Deutsch/Français/Español，语言名一律本语言显示），导航与全站核心文案即时跟随。
- **系统托盘（任务 8）**：默认关闭；开启后托盘黄瓜图标，菜单首项实时统计（运行进程 · 服务 · CPU · 内存），tooltip 周期刷新，左键唤起主窗口（Linux 无状态栏协议时静默忽略）。
- **自动更新（任务 7）**：启动 60 秒后检查 GitHub Releases latest（语义化版本比较）；有新版弹窗三选一：立即更新（打开下载页）/ 暂不 / 跳过此版本（持久化，不再提示该版本）。
- **AI 配置提示一键复制（任务 4）**：对外接口页新增按钮，生成含令牌/端口/MCP 命令/API 端点/操作步骤的完整提示词，复制给任意 AI 助手即可自动完成接入。
- **页签图标与品牌图标（任务 2）**：8 个页签配内嵌 SVG 单色图标（DPI 感知、随主题着色）；`assets/logo.svg` 品牌矢量入库；托盘/窗口/DMG 图标统一由 `tools/gen_icon.py` 确定性生成（新增 64px 托盘位图）。
- **帮助中心（任务 6）**：新增开源仓库链接（可点击打开）与作者信息。
- **列表间距（任务 1）**：进程/服务列表行距 3px、日志行距 2px + 行高微调。
- **workflow.md 入库（任务 9）**：开发工作流说明随仓库分发。

### 变更

- `Signal` 桥接模式：dropdown/slider 无回调的控件经 1 秒 UI 定时器桥接应用（语言与字号缩放），避免引入受控回调的复杂度。
- 快捷键扩展至 Ctrl+1..8（新增设置页）。
- 45 项单元/集成测试（新增 i18n 表完整性/语言名/查询、semver 比较与跳过逻辑）。

## [0.2.1] - 2026-10-28

### 修复 / 改进（macOS 分发体验）

- **DMG 标准安装视图**：左侧 Cucumber.app、右侧 `Applications` 文件夹快捷方式、背景引导图（绿色箭头指向 Applications）、128px 大图标与窗口布局（osascript 写入 .DS_Store）——打开 DMG 即可按惯例拖拽安装。
- **Gatekeeper 指引完善**：DMG 内附「首次运行说明.txt」；Release 与 README 更新为 macOS 14/15（Sonoma/Sequoia）的实际解封路径（系统设置 → 隐私与安全性 → 仍要打开；旧系统右键打开）+ `xattr -dr` 一条命令。
- 修复 `macos-bundle.sh` 参数展开闭合括号笔误导致的打包失败；`tools/gen_icon.py` 支持非正方形 PNG 并新增 `dmg_bg.png` 背景生成。

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
