//! MCP（Model Context Protocol）服务（FR-API-04，ADR-01）。
//!
//! 自实现 JSON-RPC 2.0 的 MCP 核心子集：initialize / tools/list / tools/call / ping。
//!
//! 两种传输：
//! - stdio：`cucumber --mcp`，宿主进程拉起，行分隔 JSON-RPC（MCP 规范推荐）；
//! - HTTP：`POST /mcp`（受令牌保护），单请求 JSON 响应（Streamable HTTP 简化型）。
//!
//! 工具实现复用 `api::ops`，与 HTTP API / GUI 能力对等（FR-API-02）。

use serde_json::{json, Value};

use crate::api::ops::Ops;
use crate::state::SharedState;

pub const PROTOCOL_VERSION: &str = "2024-11-05";

struct ToolDef {
    name: &'static str,
    desc: &'static str,
    schema: Value,
}

fn tools() -> Vec<ToolDef> {
    let obj = |props: Value| json!({"type": "object", "properties": props, "additionalProperties": false});
    vec![
        ToolDef {
            name: "list_processes",
            desc: "列出全部配置进程（名称、类型、状态、PID、命令）",
            schema: obj(json!({})),
        },
        ToolDef {
            name: "get_process",
            desc: "查询单个进程详情（含最近命令输出）",
            schema: obj(json!({"id": {"type": "string", "description": "进程 id，如 p1"}})),
        },
        ToolDef {
            name: "start_process",
            desc: "启动指定进程（执行其 start_command）",
            schema: obj(json!({"id": {"type": "string"}})),
        },
        ToolDef {
            name: "stop_process",
            desc: "停止指定进程（stop_command 优先，其次终止信号）",
            schema: obj(json!({"id": {"type": "string"}})),
        },
        ToolDef {
            name: "check_process",
            desc: "健康检查（执行 check_command，退出码 0 = 健康）",
            schema: obj(json!({"id": {"type": "string"}})),
        },
        ToolDef {
            name: "list_services",
            desc: "列出运行中的 Web 服务地址（含完整 URL）",
            schema: obj(json!({})),
        },
        ToolDef {
            name: "get_dashboard",
            desc: "读取仪表盘统计与 CPU/内存指标",
            schema: obj(json!({})),
        },
        ToolDef {
            name: "query_logs",
            desc: "查询进程日志（子串过滤、级别过滤、分页）",
            schema: obj(json!({
                "id": {"type": "string"},
                "source": {"type": "string", "description": "日志源名称，缺省第一个"},
                "q": {"type": "string", "description": "消息体子串"},
                "level": {"type": "string", "description": "级别，逗号分隔，如 ERROR,WARN"},
                "tag": {"type": "string"},
                "page": {"type": "integer", "minimum": 0}
            })),
        },
        ToolDef {
            name: "install_skill",
            desc: "安装黄瓜管理 skill 到指定目录（缺省 ~/.agents/skills）",
            schema: obj(json!({"target_dir": {"type": "string"}})),
        },
    ]
}

/// 处理一条 JSON-RPC 请求；notification（无 id）返回 None。
pub async fn handle_jsonrpc(state: &SharedState, body: Value) -> Value {
    let id = body.get("id").cloned();
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    let params = body.get("params").cloned().unwrap_or(json!({}));

    let result: Result<Value, String> = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "cucumber", "version": env!("APP_VERSION"), "title": "黄瓜" },
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": tools().iter().map(|t| json!({
                "name": t.name, "description": t.desc, "inputSchema": t.schema,
            })).collect::<Vec<_>>(),
        })),
        "tools/call" => call_tool(state, &params).await,
        // notifications 与未知方法
        "notifications/initialized" | "initialized" => Ok(Value::Null),
        _ => Err(format!("未知方法：{method}")),
    };

    let Some(id) = id else {
        return Value::Null; // notification：不回应
    };
    match result {
        Ok(Value::Null) => Value::Null,
        Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32602, "message": e}}),
    }
}

async fn call_tool(state: &SharedState, params: &Value) -> Result<Value, String> {
    let ops = Ops {
        state: state.clone(),
    };
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let s = |k: &str| args.get(k).and_then(Value::as_str).map(String::from);
    let n = |k: &str| args.get(k).and_then(Value::as_u64).map(|v| v as usize);

    let text: Value = match name {
        "list_processes" => json!(ops.list_processes()),
        "get_process" => ops
            .get_process(&s("id").unwrap_or_default())
            .ok_or_else(|| "进程不存在".to_string())?,
        "start_process" => ops.start(&s("id").unwrap_or_default()).await?,
        "stop_process" => ops.stop(&s("id").unwrap_or_default()).await?,
        "check_process" => ops.check(&s("id").unwrap_or_default()).await?,
        "list_services" => json!(ops.services()),
        "get_dashboard" => ops.dashboard(),
        "query_logs" => {
            let levels = s("level")
                .map(|l| {
                    l.split(',')
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            ops.query_logs(
                &s("id").unwrap_or_default(),
                s("source"),
                s("q"),
                levels,
                s("tag"),
                None,
                None,
                n("page").unwrap_or(0),
                None,
            )
            .await?
        }
        "install_skill" => ops.install_skill(s("target_dir"))?,
        _ => return Err(format!("未知工具：{name}")),
    };
    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&text).unwrap_or_default()}],
        "isError": false,
    }))
}

/// stdio 模式：每行一条 JSON-RPC；响应按行写 stdout（FR-API-03：stdio 优先）。
pub async fn stdio_server(state: SharedState) -> i32 {
    use std::io::{BufRead, Write};

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(body) = serde_json::from_str::<Value>(line) else {
            let _ = writeln!(
                out,
                "{}",
                json!({
                    "jsonrpc": "2.0", "id": Value::Null,
                    "error": {"code": -32700, "message": "解析错误"}
                })
            );
            let _ = out.flush();
            continue;
        };
        let resp = handle_jsonrpc(&state, body).await;
        if !resp.is_null() {
            let _ = writeln!(out, "{resp}");
            let _ = out.flush();
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{Config, ProcessConfig};
    use axum::http::StatusCode;
    use tower::ServiceExt;

    fn test_state() -> SharedState {
        let mut cfg = Config::default();
        cfg.general.auth_token = Some(crate::core::config::generate_token());
        cfg.upsert_process(ProcessConfig {
            id: "p1".into(),
            name: "示例".into(),
            ..Default::default()
        });
        crate::state::AppState::new(
            cfg,
            std::path::PathBuf::from("/tmp/cucumber-mcp-test/config.toml"),
        )
    }

    #[tokio::test]
    async fn initialize_and_tools_list() {
        let state = test_state();
        let init = handle_jsonrpc(
            &state,
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        )
        .await;
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(init["result"]["serverInfo"]["name"], "cucumber");

        let list = handle_jsonrpc(
            &state,
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        )
        .await;
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for expect in [
            "list_processes",
            "start_process",
            "query_logs",
            "install_skill",
        ] {
            assert!(names.contains(&expect), "缺少工具 {expect}");
        }
    }

    #[tokio::test]
    async fn tools_call_roundtrip() {
        let state = test_state();
        let r = handle_jsonrpc(
            &state,
            json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "list_processes", "arguments": {}}
            }),
        )
        .await;
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed[0]["name"], "示例");

        // 不存在的进程 → error 响应
        let bad = handle_jsonrpc(
            &state,
            json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": {"name": "start_process", "arguments": {"id": "nope"}}
            }),
        )
        .await;
        assert!(bad.get("error").is_some());
    }

    #[tokio::test]
    async fn notification_no_response() {
        let state = test_state();
        let r = handle_jsonrpc(
            &state,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        )
        .await;
        assert!(r.is_null());
    }

    #[tokio::test]
    async fn mcp_http_endpoint_auth_and_call() {
        let state = test_state();
        let token = state
            .config
            .read()
            .unwrap()
            .general
            .auth_token
            .clone()
            .unwrap();
        let app = crate::api::http::full_router(state.clone());
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::to_string(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
                    .unwrap(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 无令牌 → 401
        let app2 = crate::api::http::full_router(state);
        let req2 = axum::http::Request::builder()
            .method("POST")
            .uri("/mcp")
            .body(axum::body::Body::from("{}"))
            .unwrap();
        let resp2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);
    }
}
