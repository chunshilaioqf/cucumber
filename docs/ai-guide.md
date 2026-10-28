# 黄瓜 · AI 接入说明

黄瓜在启动后于本机回环地址开放三类接口，能力与 GUI 完全对等。端口与令牌见「对外接口」页面，或读取配置文件 `~/.config/cucumber/config.toml`（macOS：`~/Library/Application Support/cucumber/`；Windows：`%APPDATA%\cucumber\`）。

## 认证（NFR-SEC-01）

- 所有接口仅绑定 `127.0.0.1`。
- HTTP/MCP：请求头 `Authorization: Bearer <token>` 或查询参数 `?token=<token>`。
- WebSocket：`ws://127.0.0.1:<ws_port>/ws?token=<token>`。
- 令牌首次启动生成并持久化；不要写进代码或日志。

## MCP（推荐）

- **stdio**：宿主进程拉起 `cucumber --mcp`，按行收发 JSON-RPC 2.0（MCP 规范 2024-11-05 子集：`initialize` / `tools/list` / `tools/call` / `ping`）。
- **HTTP**：`POST http://127.0.0.1:<http_port>/mcp`，请求体为单条 JSON-RPC，需令牌。

工具列表：`list_processes`、`get_process`、`start_process`、`stop_process`、`check_process`、`list_services`、`get_dashboard`、`query_logs`、`install_skill`。

示例（tools/call）：

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_processes","arguments":{}}}
```

## HTTP API 一览

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | /api/health | 免认证健康检查 |
| GET | /api/processes | 列出全部配置进程 |
| POST | /api/processes | 新建（体为进程配置 JSON） |
| PUT | /api/processes/{id} | 更新 |
| DELETE | /api/processes/{id} | 删除 |
| POST | /api/processes/{id}/start | 启动 |
| POST | /api/processes/{id}/stop | 停止 |
| POST | /api/processes/{id}/check | 健康检查 |
| POST | /api/processes/{id}/install_deps | 安装依赖 |
| GET | /api/dashboard | 统计 + CPU/内存 |
| GET | /api/services | 运行中 Web 服务 |
| GET | /api/logs/{id}?source=&q=&level=&tag=&page= | 日志查询 |
| POST | /api/skills/install | 安装 skill（体 `{"target_dir": "…"}` 可选） |

示例：

```bash
TOKEN=$(grep auth_token ~/.config/cucumber/config.toml | cut -d'"' -f2)
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18080/api/dashboard
```

## WebSocket

连接后每 3 秒收到一次 dashboard 快照（JSON）；可发送控制消息：

```json
{"type":"start","id":"p1"}
{"type":"stop","id":"p1"}
{"type":"check","id":"p1"}
{"type":"dashboard"}
```

## Skill 安装

「对外接口」页一键安装，或：

```bash
curl -s -X POST -H "Authorization: Bearer $TOKEN" \
  -d '{"target_dir":"~/.agents/skills"}' \
  http://127.0.0.1:18080/api/skills/install
```

skill 是纯文本目录（`SKILL.md`，YAML frontmatter + Markdown），禁止可执行脚本；安装清单写入目标目录 `.skill-lock.json`。
