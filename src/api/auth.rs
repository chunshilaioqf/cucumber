//! 本地令牌认证（FR-API-03，NFR-SEC-01）：仅绑定 127.0.0.1，强制 Bearer/query 令牌。

use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::state::SharedState;

/// 校验：`Authorization: Bearer <token>` 或 `?token=<token>`。
pub fn token_matches(state: &SharedState, req: &Request<axum::body::Body>) -> bool {
    let Some(expect) = state
        .config
        .read()
        .ok()
        .and_then(|c| c.general.auth_token.clone())
    else {
        return false;
    };
    if let Some(auth) = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
    {
        if auth.trim() == format!("Bearer {expect}") {
            return true;
        }
    }
    if let Some(q) = req.uri().query() {
        for pair in q.split('&') {
            if let Some(tok) = pair.strip_prefix("token=") {
                if tok == expect {
                    return true;
                }
            }
        }
    }
    false
}

/// 保护路由的中间件；/api/health 免认证。
pub async fn require_token(
    State(state): State<SharedState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if token_matches(&state, &req) {
        next.run(req).await
    } else {
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "未授权：缺少或错误的本地令牌",
        )
            .into_response()
    }
}
