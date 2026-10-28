//! WebSocket：实时事件推送 + 控制消息（FR-API-02）。
//!
//! 认证复用 HTTP 中间件（/ws 在受保护路由组内）；连接后：
//! - 每 refresh_secs 推送一次 dashboard 快照；
//! - 广播通道上的进程/服务事件即时转发；
//! - 接收 `{"type":"start|stop|check|dashboard","id":"p1"}` 控制消息。

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};

use crate::api::ops::Ops;
use crate::state::SharedState;

pub async fn upgrade(State(state): State<SharedState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle(state, socket))
        .into_response()
}

async fn handle(state: SharedState, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let mut events = state.events.subscribe();
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(state.refresh_secs()));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            ev = events.recv() => {
                if let Ok(ev) = ev {
                    if sender.send(Message::Text(ev.into())).await.is_err() {
                        break;
                    }
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text = text.as_str().to_string();
                        if !handle_control(&state, &mut sender, &text).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(p))) => {
                        if sender.send(Message::Pong(p)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                }
            }
            _ = tick.tick() => {
                let payload = serde_json::to_string(&Ops { state: state.clone() }.dashboard())
                    .unwrap_or_else(|_| "{}".into());
                if sender.send(Message::Text(payload.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// 处理控制消息；返回 false 表示应断开。
async fn handle_control(
    state: &SharedState,
    sender: &mut SplitSink<WebSocket, Message>,
    text: &str,
) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return true; // 非 JSON 忽略
    };
    let ops = Ops {
        state: state.clone(),
    };
    let r = match (
        v.get("type").and_then(|t| t.as_str()),
        v.get("id").and_then(|i| i.as_str()),
    ) {
        (Some("start"), Some(id)) => Some(ops.start(id).await),
        (Some("stop"), Some(id)) => Some(ops.stop(id).await),
        (Some("check"), Some(id)) => Some(ops.check(id).await),
        (Some("dashboard"), _) => Some(Ok(ops.dashboard())),
        _ => None,
    };
    if let Some(res) = r {
        let payload = match res {
            Ok(v) => serde_json::to_string(&serde_json::json!({ "result": v })),
            Err(e) => serde_json::to_string(&serde_json::json!({ "error": e })),
        }
        .unwrap_or_else(|_| "{}".into());
        if sender.send(Message::Text(payload.into())).await.is_err() {
            return false;
        }
    }
    true
}
