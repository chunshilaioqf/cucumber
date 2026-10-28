---
name: cucumber-manager
description: 通过黄瓜（cucumber）本地 MCP/HTTP 接口列出、启动、停止、检查本机进程与服务，查询进程日志与仪表盘指标。
version: 0.1.0
---

# 黄瓜进程管理 Skill

> 教 AI Agent 通过黄瓜（cucumber）的 MCP / HTTP API 管理本地进程与服务。

## 何时使用

用户要求在本地「启动/停止/查看某个进程或服务」「看看本机跑了什么 Web 服务」「查某个进程的日志」「看看 CPU/内存」时，使用本 skill。

## 接入方式

黄瓜在运行后会在本机回环地址开放三类接口（端口与令牌以「对外接口」页面或 `~/.config/cucumber/config.toml` 为准）：

1. **MCP（推荐）**：宿主以 stdio 拉起 `cucumber --mcp`，或 POST `http://127.0.0.1:<http_port>/mcp`（JSON-RPC 2.0）。
2. **HTTP API**：`http://127.0.0.1:<http_port>/api/...`，请求头 `Authorization: Bearer <token>`。
3. **WebSocket**：`ws://127.0.0.1:<ws_port>/ws?token=<token>`，接收进程状态与指标事件。

## MCP 工具列表

| 工具 | 参数 | 说明 |
|---|---|---|
| `list_processes` | 无 | 列出全部配置进程（含状态/PID/类型） |
| `get_process` | `id` | 单个进程详情（含最近命令输出） |
| `start_process` | `id` | 执行该进程的 start_command |
| `stop_process` | `id` | stop_command 优先，其次终止信号 |
| `check_process` | `id` | 执行 check_command，退出码 0 = 健康 |
| `list_services` | 无 | 运行中的 Web 服务地址列表 |
| `get_dashboard` | 无 | 进程统计 + CPU/内存指标 |
| `query_logs` | `id`, `source?`, `q?`, `level?`, `page?` | 日志查询（分页） |
| `install_skill` | `target_dir?` | 安装本 skill 到指定目录 |

## 操作约定

- 启动/停止前先 `list_processes` 确认目标 `id` 与当前状态，避免重复启动。
- 停止失败时读取 `get_process` 的 `last_output`（命令 stdout/stderr）诊断原因。
- 查询日志时优先给用户看 ERROR/WARN 级别：`query_logs` 带 `level=ERROR,WARN`。
- 未经用户确认，不要删除（DELETE）任何进程配置。
