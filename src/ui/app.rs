//! 主窗口布局：菜单栏 + 页面标签（8 页，带 SVG 图标）+ 更新提醒对话框（任务 2/7）。

use windui::prelude::*;

use super::{about, api_page, dashboard, docs, icons, logs, processes, services, settings, UiCtx};
use crate::core::update;

/// 组装主界面（上下布局：menu_bar + tabs_icons 页面区）。
pub fn build(ui_ref: &UiCtx) -> Element {
    let file = {
        let ui = ui_ref.clone();
        move || {
            vec![
                MenuItem::run(
                    ui.t.c_add.get(),
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
            let cur = ui.page.get();
            let titles = [
                ui.t.nav_dashboard.get(),
                ui.t.nav_processes.get(),
                ui.t.nav_services.get(),
                ui.t.nav_logs.get(),
                ui.t.nav_api.get(),
                ui.t.nav_docs.get(),
                ui.t.nav_help.get(),
                ui.t.nav_settings.get(),
            ];
            titles
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
                    ui.t.nav_docs.get(),
                    {
                        let ui = ui.clone();
                        move |_ctx| ui.page.set(5)
                    },
                    false,
                ),
                MenuItem::run(
                    format!("{}(A)…", ui.t.nav_help.get()),
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

    // 页签：标题用文案信号（语言切换自动跟随），图标为内嵌 SVG（DPI 感知）
    let tabs = Element::tabs_icons(
        ui_ref.page,
        vec![
            (
                ui_ref.t.nav_dashboard,
                icons::svg_icon(icons::DASHBOARD),
                dashboard::page(ui_ref),
            ),
            (
                ui_ref.t.nav_processes,
                icons::svg_icon(icons::PROCESSES),
                processes::page(ui_ref),
            ),
            (
                ui_ref.t.nav_services,
                icons::svg_icon(icons::SERVICES),
                services::page(ui_ref),
            ),
            (
                ui_ref.t.nav_logs,
                icons::svg_icon(icons::LOGS),
                logs::page(ui_ref),
            ),
            (
                ui_ref.t.nav_api,
                icons::svg_icon(icons::API),
                api_page::page(ui_ref),
            ),
            (
                ui_ref.t.nav_docs,
                icons::svg_icon(icons::DOCS),
                docs::page(ui_ref),
            ),
            (
                ui_ref.t.nav_help,
                icons::svg_icon(icons::HELP),
                about::page(ui_ref),
            ),
            (
                ui_ref.t.nav_settings,
                icons::svg_icon(icons::SETTINGS),
                settings::page(ui_ref),
            ),
        ],
    )
    .weight(1.0);

    Element::stack()
        .child(
            Element::col()
                .fill()
                .bg_role(Role::Bg)
                .child(menu)
                .child(tabs),
        )
        .child(update_dialog(ui_ref))
}

// ---------- 更新提醒（任务 7） ----------

fn update_dialog(ui: &UiCtx) -> Element {
    let body = Element::col()
        .width(400)
        .spacing(12)
        .child(
            Element::label_signal(signal(String::new())).visible_when(move || false), // 占位保持布局
        )
        .child(
            Element::col()
                .spacing(4)
                .child(Element::label_signal(ui.update_ver).font_size(13.0))
                .child(
                    Element::label(update::REPO_URL)
                        .fg(super::kit::colors::BLUE)
                        .font_size(12.0),
                ),
        )
        .child(
            Element::hstack()
                .spacing(10)
                // 立即更新：打开下载页
                .child(Element::button(ui.t.u_now).on_click({
                    let ui = ui.clone();
                    move |_| {
                        let url = ui.update_url.get();
                        let (rt, tx) = (ui.rt.clone(), ui.tx.clone());
                        let toast = crate::ui::i18n::tr(ui.lang.get(), "u.gotodl");
                        rt.spawn(async move {
                            let _ = crate::platform::browser::open_url(&url).await;
                            let _ = tx.send(crate::ui::UiMsg::ToastOk(toast));
                        });
                        ui.update_show.set(false);
                    }
                }))
                // 暂不：下次启动再提示
                .child(Element::button(ui.t.u_later).neutral().on_click({
                    let ui = ui.clone();
                    move |_| ui.update_show.set(false)
                }))
                // 跳过此版本：写配置，不再提示该版本
                .child(Element::button(ui.t.u_skip).outline().on_click({
                    let ui = ui.clone();
                    move |_| {
                        let ver = ui.update_ver.get();
                        if let Ok(mut c) = ui.app.config.write() {
                            c.general.skip_update = Some(ver);
                        }
                        let _ = ui.app.save_config();
                        ui.update_show.set(false);
                    }
                })),
        );

    Element::dialog_panel(
        ui.update_show,
        ui.t.u_title,
        430,
        {
            let ui = ui.clone();
            move |_ctx| ui.update_show.set(false)
        },
        body,
        Element::label("").height(0),
    )
}
