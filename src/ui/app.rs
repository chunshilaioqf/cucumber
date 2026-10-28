//! 主窗口布局：菜单栏 + 页面标签（FR-DASH-03：仪表盘为默认页且只读）。

use windui::prelude::*;

use super::{about, api_page, dashboard, docs, logs, processes, services, UiCtx};

/// 组装主界面（上下布局：menu_bar + tabs 页面区）。
pub fn build(ui_ref: &UiCtx) -> Element {
    let file = {
        let ui = ui_ref.clone();
        move || {
            vec![
                MenuItem::run(
                    "添加进程…",
                    {
                        let ui = ui.clone();
                        move |_ctx| {
                            processes::open_editor_new(&ui);
                            ui.page.set(1);
                        }
                    },
                    false,
                )
                .shortcut("Ctrl+N")
                .mnemonic('A'),
                MenuItem::separator(),
                MenuItem::run("退出(X)", |ctx| ctx.request_close(), false)
                    .shortcut("Alt+F4")
                    .mnemonic('X'),
            ]
        }
    };
    let view = {
        let ui = ui_ref.clone();
        move || {
            let pages = [
                "仪表盘",
                "进程",
                "所有服务",
                "日志",
                "对外接口",
                "使用说明",
                "帮助",
            ];
            let cur = ui.page.get();
            pages
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let ui = ui.clone();
                    MenuItem::run(
                        format!("{name}（Ctrl+{}）", i + 1),
                        move |_ctx| ui.page.set(i),
                        i == cur,
                    )
                    .mnemonic((b'1' + i as u8) as char)
                })
                .chain(std::iter::once(MenuItem::separator()))
                .collect::<Vec<_>>()
        }
    };
    let help = {
        let ui = ui_ref.clone();
        move || {
            vec![
                MenuItem::run(
                    "使用说明",
                    {
                        let ui = ui.clone();
                        move |_ctx| ui.page.set(5)
                    },
                    false,
                ),
                MenuItem::run(
                    "关于黄瓜(A)…",
                    {
                        let ui = ui.clone();
                        move |_ctx| ui.page.set(6)
                    },
                    false,
                )
                .mnemonic('A'),
            ]
        }
    };

    let menu = Element::menu_bar(vec![
        MenuBarEntry::new("文件(F)", file).mnemonic('F'),
        MenuBarEntry::new("视图(V)", view).mnemonic('V'),
        MenuBarEntry::new("帮助(H)", help).mnemonic('H'),
    ])
    .height(28)
    .padding_xy(4, 0)
    .bg_role(Role::Surface);

    let tabs = Element::tabs(
        ui_ref.page,
        vec![
            ("仪表盘", dashboard::page(ui_ref)),
            ("进程", processes::page(ui_ref)),
            ("所有服务", services::page(ui_ref)),
            ("日志", logs::page(ui_ref)),
            ("对外接口", api_page::page(ui_ref)),
            ("使用说明", docs::page(ui_ref)),
            ("帮助", about::page(ui_ref)),
        ],
    )
    .weight(1.0);

    Element::col()
        .fill()
        .bg_role(Role::Bg)
        .child(menu)
        .child(tabs)
}
