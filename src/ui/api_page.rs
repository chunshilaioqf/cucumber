//! 对外接口页（FR-API-01~04，FR-SKILL）：端口与令牌展示、skill 一键安装。

use windui::prelude::*;

use super::kit;
use super::{spawn_op, UiCtx};

pub fn page(ui_ref: &UiCtx) -> Element {
    let ui = ui_ref.clone();
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
            .child(mcp_hint),
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
