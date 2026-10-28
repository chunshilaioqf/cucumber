//! 所有服务页（FR-SERV-01~03）：运行中 Web 服务集中列表，可复制/打开 URL。

use windui::prelude::*;

use super::{SvcRow, UiCtx};
use crate::platform::browser;

pub fn page(ui_ref: &UiCtx) -> Element {
    let ui = ui_ref.clone();
    let hint =
        Element::label("集中展示由黄瓜管理、类型为 Web 服务且运行中的进程地址（FR-SERV-01）。")
            .fg_role(Role::TextMuted)
            .font_size(12.0);

    let ui_for_rows = ui.clone();
    let list = Element::list_signal(
        ui.svc,
        |r: &SvcRow| r.url.clone(),
        move |r: SvcRow| row(&ui_for_rows, r),
    )
    .weight(1.0);

    let empty = {
        let svc = ui.svc;
        Element::label("当前没有运行中的 Web 服务。")
            .fg_role(Role::TextMuted)
            .visible_when(move || svc.with(|r| r.is_empty()))
    };

    Element::col()
        .fill()
        .padding_xy(16, 8)
        .spacing(10)
        .child(hint)
        .child(empty)
        .child(list)
}

fn row(ui: &UiCtx, r: SvcRow) -> Element {
    let mut actions =
        Element::hstack()
            .spacing(6)
            .child(Element::button("复制 URL").small().outline().on_click({
                let url = r.url.clone();
                move |ctx| {
                    ctx.clipboard_set(&url);
                    ctx.toast_ok("已复制到剪贴板");
                }
            }));
    if r.openable {
        actions = actions.child(Element::button("打开").small().on_click({
            let (rt, tx) = (ui.rt.clone(), ui.tx.clone());
            let url = r.url.clone();
            move |_| {
                let url2 = url.clone();
                let tx2 = tx.clone();
                rt.spawn(async move {
                    if let Err(e) = browser::open_url(&url2).await {
                        let _ =
                            tx2.send(crate::ui::UiMsg::ToastErr(format!("打开浏览器失败：{e}")));
                    }
                });
            }
        }));
    }

    Element::row()
        .width_match()
        .cross(Align::Center)
        .padding_xy(10, 8)
        .bg_role(Role::Surface)
        .corner(8.0)
        .border_role(Role::Border, 1)
        .spacing(10)
        .animate_item()
        .child(
            Element::col()
                .width(220)
                .spacing(2)
                .child(Element::label(r.proc_name.clone()).font_weight(600))
                .child(
                    Element::label(format!("PID {}", r.pid_text))
                        .fg_role(Role::TextMuted)
                        .font_size(12.0),
                ),
        )
        .child(Element::label(r.url.clone()).font_size(12.5))
        .child(Element::flex_spacer())
        .child(actions)
}
