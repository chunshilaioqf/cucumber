//! api：对外接口层（HTTP / WebSocket / MCP / 认证 / 统一操作）。

pub mod auth;
pub mod http;
pub mod mcp;
pub mod ops;
pub mod ws;

/// 启动全部对外接口（仅 127.0.0.1），返回 (http_port, ws_port) 并回写状态。
pub async fn start_all(state: crate::state::SharedState) -> Result<(u16, u16), String> {
    let (http_pref, ws_pref) = {
        let c = state.config.read().map_err(|e| e.to_string())?;
        (c.general.http_port, c.general.ws_port)
    };
    let http = http::serve_http(state.clone(), http_pref).await?;
    let ws = http::serve_ws(state.clone(), ws_pref).await?;
    // 实际端口持久化（FR-API-01）
    {
        let mut c = state.config.write().map_err(|e| e.to_string())?;
        c.general.http_port = http;
        c.general.ws_port = ws;
    }
    let _ = state.save_config();
    Ok((http, ws))
}
