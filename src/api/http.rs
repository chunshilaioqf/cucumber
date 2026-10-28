//! axum HTTP API（FR-API-01/02/04）：RESTful，覆盖 GUI 全部能力。

use std::net::SocketAddr;
use std::sync::atomic::Ordering;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::require_token;
use crate::api::mcp;
use crate::api::ops::Ops;
use crate::api::ws;
use crate::state::SharedState;

/// HTTP API 路由（不含 /ws）。
pub fn http_router(state: SharedState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .merge(protected(state.clone()))
        .with_state(state)
}

/// WS 路由（独立端口，同样强制令牌）。
pub fn ws_router(state: SharedState) -> Router {
    protected(state.clone())
        .route("/ws", get(ws::upgrade))
        .with_state(state)
}

/// 测试/调试便利：全量路由（HTTP + WS 同端口）。
#[allow(dead_code)] // 单元测试与调试用
pub fn full_router(state: SharedState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .merge(protected(state.clone()))
        .route("/ws", get(ws::upgrade))
        .with_state(state)
}

fn protected(state: SharedState) -> Router<SharedState> {
    Router::new()
        .route("/api/processes", get(list_processes).post(create_process))
        .route(
            "/api/processes/{id}",
            get(get_process).put(update_process).delete(delete_process),
        )
        .route("/api/processes/{id}/start", post(start_proc))
        .route("/api/processes/{id}/stop", post(stop_proc))
        .route("/api/processes/{id}/check", post(check_proc))
        .route("/api/processes/{id}/install_deps", post(install_deps))
        .route("/api/dashboard", get(dashboard))
        .route("/api/services", get(services))
        .route("/api/logs/{id}", get(query_logs))
        .route("/api/skills/install", post(install_skill))
        .route("/mcp", post(mcp_http))
        .layer(axum::middleware::from_fn_with_state(state, require_token))
}

/// 启动监听，返回实际端口；is_ws 决定写入 ws_port 还是 http_port。
async fn spawn_listener(
    state: SharedState,
    router: Router,
    preferred: u16,
    is_ws: bool,
) -> Result<u16, String> {
    let app = router;
    let addr = SocketAddr::from(([127, 0, 0, 1], preferred));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("绑定 {addr} 失败：{e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    if is_ws {
        state.ws_port.store(port, Ordering::Relaxed);
    } else {
        state.http_port.store(port, Ordering::Relaxed);
    }
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(port)
}

/// 启动 HTTP 服务，返回实际端口。
pub async fn serve_http(state: SharedState, preferred: u16) -> Result<u16, String> {
    spawn_listener(state.clone(), http_router(state), preferred, false).await
}

/// 启动 WS 服务，返回实际端口。
pub async fn serve_ws(state: SharedState, preferred: u16) -> Result<u16, String> {
    spawn_listener(state.clone(), ws_router(state), preferred, true).await
}

// ---------- handlers ----------

async fn health() -> impl IntoResponse {
    Json(json!({
        "app": "cucumber", "name": "黄瓜",
        "version": env!("APP_VERSION"),
        "git_sha": env!("GIT_SHA"),
        "platform": std::env::consts::OS,
    }))
}

async fn list_processes(State(state): State<SharedState>) -> impl IntoResponse {
    Json(Ops { state }.list_processes())
}

async fn create_process(
    State(state): State<SharedState>,
    body: Option<Json<crate::core::config::ProcessConfig>>,
) -> Response {
    match body {
        Some(Json(c)) => respond(Ops { state }.upsert_process(c)),
        None => err_response("请求体应为进程配置 JSON"),
    }
}

async fn update_process(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(mut cfg): Json<crate::core::config::ProcessConfig>,
) -> Response {
    cfg.id = id;
    respond(Ops { state }.upsert_process(cfg))
}

async fn delete_process(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    respond(Ops { state }.delete_process(&id))
}

async fn get_process(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    match (Ops { state }.get_process(&id)) {
        Some(v) => Json(v).into_response(),
        None => err_response(&format!("进程不存在：{id}")),
    }
}

async fn start_proc(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    respond(Ops { state }.start(&id).await)
}
async fn stop_proc(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    respond(Ops { state }.stop(&id).await)
}
async fn check_proc(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    respond(Ops { state }.check(&id).await)
}
async fn install_deps(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    respond(Ops { state }.install_deps(&id).await)
}

async fn dashboard(State(state): State<SharedState>) -> impl IntoResponse {
    Json(Ops { state }.dashboard())
}

async fn services(State(state): State<SharedState>) -> impl IntoResponse {
    Json(Ops { state }.services())
}

#[derive(Deserialize)]
struct LogQuery {
    source: Option<String>,
    q: Option<String>,
    /// 逗号分隔：ERROR,WARN
    level: Option<String>,
    tag: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
    ts_from: Option<i64>,
    ts_to: Option<i64>,
}

async fn query_logs(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<LogQuery>,
) -> Response {
    let levels = q
        .level
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    respond(
        Ops { state }
            .query_logs(
                &id,
                q.source,
                q.q,
                levels,
                q.tag,
                q.ts_from,
                q.ts_to,
                q.page.unwrap_or(0),
                q.page_size,
            )
            .await,
    )
}

#[derive(Deserialize)]
struct SkillBody {
    target_dir: Option<String>,
}

async fn install_skill(
    State(state): State<SharedState>,
    body: Option<Json<SkillBody>>,
) -> Response {
    respond(Ops { state }.install_skill(body.and_then(|Json(b)| b.target_dir)))
}

async fn mcp_http(State(state): State<SharedState>, Json(body): Json<Value>) -> Response {
    let resp = mcp::handle_jsonrpc(&state, body).await;
    Json(resp).into_response()
}

fn respond(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

fn err_response(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, msg.to_string()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{Config, ProcessConfig, ProcessType};
    use axum::http::StatusCode;
    use tower::ServiceExt;

    fn test_state() -> SharedState {
        let mut cfg = Config::default();
        cfg.general.auth_token = Some(crate::core::config::generate_token());
        cfg.upsert_process(ProcessConfig {
            id: "p1".into(),
            name: "示例".into(),
            ptype: ProcessType::WebService,
            ..Default::default()
        });
        crate::state::AppState::new(
            cfg,
            std::path::PathBuf::from("/tmp/cucumber-api-test/config.toml"),
        )
    }

    #[tokio::test]
    async fn health_open_and_others_protected() {
        let state = test_state();
        let app = full_router(state.clone());

        // health 免认证
        let req = axum::http::Request::builder()
            .uri("/api/health")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 无令牌 → 401
        let req = axum::http::Request::builder()
            .uri("/api/processes")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // 带令牌 → 200
        let token = state
            .config
            .read()
            .unwrap()
            .general
            .auth_token
            .clone()
            .unwrap();
        let req = axum::http::Request::builder()
            .uri("/api/processes")
            .header("authorization", format!("Bearer {token}"))
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn crud_and_dashboard() {
        let state = test_state();
        let token = state
            .config
            .read()
            .unwrap()
            .general
            .auth_token
            .clone()
            .unwrap();
        let app = full_router(state.clone());

        // dashboard
        let req = axum::http::Request::builder()
            .uri("/api/dashboard")
            .header("authorization", format!("Bearer {token}"))
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["processes_total"], 1);
        assert_eq!(v["web_services_configured"], 1);

        // 新建
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/processes")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::to_string(&ProcessConfig {
                    id: "".into(),
                    name: "新建".into(),
                    ptype: ProcessType::Generic,
                    start_command: Some("true".into()),
                    ..Default::default()
                })
                .unwrap(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 列表应有两个
        let cfg = state.config.read().unwrap();
        assert_eq!(cfg.processes.len(), 2);
    }
}
