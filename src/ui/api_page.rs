//! 对外接口页（FR-API-01~04，FR-SKILL）：端口与令牌展示、skill 一键安装。

use windui::prelude::*;

use super::kit;
use super::{spawn_op, UiCtx};

pub fn page(ui_ref: &UiCtx) -> Element {
    let ui = ui_ref.clone();
    let f = |key: &str| crate::ui::i18n::tr(ui.lang.get(), key);
    let status = Element::field(
        "监听状态（仅 127.0.0.1）",
        Element::col()
            .spacing(4)
            .child(Element::label_signal(ui.api_status).font_size(12.5)),
    );

    // 令牌：默认脱敏，切换显示（NFR-SEC-01：令牌不出现在日志，UI 可查看）
    let token_line = {
        let app = ui.app.clone();
        let shown = token_display(&app, ui.token_show.get());
        Element::hstack()
            .spacing(8)
            .child(
                Element::text_input(signal(shown), "未生成")
                    .read_only()
                    .width(460)
                    .font_size(12.0),
            )
            .child(Element::button("显示/隐藏").small().neutral().on_click({
                let ui = ui.clone();
                move |_| ui.token_show.update(|v| *v = !*v)
            }))
            .child(Element::button("复制").small().outline().on_click({
                let app = ui.app.clone();
                let ui = ui.clone();
                move |ctx| {
                    let t = app
                        .config
                        .read()
                        .unwrap()
                        .general
                        .auth_token
                        .clone()
                        .unwrap_or_default();
                    ctx.clipboard_set(&t);
                    let _ = ui.tx.send(crate::ui::UiMsg::ToastOk("令牌已复制".into()));
                }
            }))
    };
    let token = Element::field(
        "本地令牌（Authorization: Bearer … 或 ?token=…）",
        token_line,
    );

    // skill 安装（FR-SKILL-01~03）
    let skill = Element::card(
        "Skill 一键安装",
        Element::col()
            .spacing(8)
            .width_match()
            .child(
                Element::label(
                    "内置 skill：cucumber-manager（目录 + SKILL.md，无脚本，NFR-SEC-03）。\
                     安装到 ~/.agents/skills/ 或自定义目录，维护 .skill-lock.json 防重复。",
                )
                .font_size(12.5),
            )
            .child(
                Element::hstack()
                    .spacing(8)
                    .child(
                        Element::text_input(ui.skill_dir, "目标目录（留空 = ~/.agents/skills）")
                            .width(360),
                    )
                    .child(Element::button("安装 skill").on_click({
                        let ui = ui.clone();
                        move |_| {
                            let target = ui.skill_dir.get();
                            let target = target.trim().to_string();
                            let target = if target.is_empty() {
                                None
                            } else {
                                Some(target)
                            };
                            let ui2 = ui.clone();
                            let (tx, rt) = (ui.tx.clone(), ui.rt.clone());
                            spawn_op(tx, &rt, "skill 安装成功".into(), move || {
                                let state = ui2.app.clone();
                                let t2 = target.clone();
                                async move {
                                    crate::api::ops::Ops { state }.install_skill(t2).map(|_| ())
                                }
                            });
                        }
                    })),
            ),
    );

    // AI 配置提示一键复制（任务 4）：含令牌与端点的完整提示词，发给任意 AI 即可自动接入
    let ai_prompt = Element::card(
        f("a.ai"),
        Element::col()
            .spacing(8)
            .width_match()
            .child(
                Element::label(f("a.aihint"))
                    .fg_role(Role::TextMuted)
                    .font_size(12.5),
            )
            .child(Element::button(f("a.ai")).on_click({
                let ui = ui.clone();
                move |ctx| {
                    let prompt = build_ai_prompt(&ui);
                    ctx.clipboard_set(&prompt);
                    ctx.toast_ok("AI 配置提示已复制，粘贴给任意 AI 助手即可");
                }
            })),
    );

    let mcp_hint = Element::card(
        "MCP 接入",
        Element::col().spacing(4)
            .child(
                Element::label("stdio（推荐，由 AI 宿主拉起）：")
                    .font_weight(600)
                    .font_size(12.5),
            )
            .child(Element::label("\tcucumber --mcp").font_size(12.0))
            .child(
                Element::label("HTTP（需令牌）：POST http://127.0.0.1:<http端口>/mcp")
                    .font_size(12.0),
            )
            .child(
                Element::label("工具集与 GUI 能力对等：list/start/stop/check 进程、服务、仪表盘、日志、装 skill")
                    .fg(kit::colors::GRAY)
                    .font_size(12.0),
            ),
    );

    Element::scroll().child(
        Element::col()
            .fill()
            .padding(16)
            .spacing(12)
            .child(status)
            .child(token)
            .child(skill)
            .child(mcp_hint)
            .child(ai_prompt),
    )
}

/// 生成给 AI 助手的完整配置提示词（含令牌与端点，任务 4）。
fn build_ai_prompt(ui: &UiCtx) -> String {
    let cfg = ui.app.config.read().unwrap();
    let token = cfg.general.auth_token.clone().unwrap_or_default();
    drop(cfg);
    let http = ui.app.http_port.load(std::sync::atomic::Ordering::Relaxed);
    let ws = ui.app.ws_port.load(std::sync::atomic::Ordering::Relaxed);
    format!(
        r#"请帮我接入本机已运行的「黄瓜 cucumber」进程管理工具，按以下信息完成配置并验证：

## 接入信息（本机回环地址）
- HTTP API 基址：http://127.0.0.1:{http}
- WebSocket：ws://127.0.0.1:{ws}/ws?token={token}
- 认证方式：所有请求携带请求头 `Authorization: Bearer {token}`（或查询参数 `?token={token}`）
- MCP stdio 命令：`cucumber --mcp`（宿主直接拉起进程即可，无需令牌）
- MCP HTTP 端点：POST http://127.0.0.1:{http}/mcp （单条 JSON-RPC 2.0，需上述令牌）

## 可用能力（与 GUI 对等）
- 进程管理：GET/POST/PUT/DELETE /api/processes（增删改查）；POST /api/processes/{{id}}/start|stop|check|install_deps（启动/停止/健康检查/装依赖）
- 仪表盘：GET /api/dashboard（进程统计 + CPU/内存/磁盘/网络指标）
- 服务列表：GET /api/services（运行中 Web 服务地址）
- 日志查询：GET /api/logs/{{id}}?source=&q=&level=ERROR,WARN&tag=&page=0&page_size=100
- MCP 工具集：list_processes / get_process / start_process / stop_process / check_process / list_services / get_dashboard / query_logs / install_skill

## 请依次完成
1. 用上面的令牌请求 `GET /api/health` 与 `GET /api/dashboard` 验证连通。
2. （如需）把 MCP 端点或 stdio 命令注册进你的工具配置。
3. 回报验证结果；之后按我的指令管理进程、服务与日志。

注意：令牌只在本机使用，不要写入公开仓库或日志。"#,
        http = http,
        ws = ws,
        token = token
    )
}

fn token_display(app: &crate::state::SharedState, shown: bool) -> String {
    let cfg = app.config.read().unwrap();
    match (&cfg.general.auth_token, shown) {
        (Some(t), true) => t.clone(),
        (Some(_), false) => cfg.masked_token(),
        (None, _) => "未生成".into(),
    }
}
