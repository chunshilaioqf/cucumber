//! 仪表盘（任务 5 重做）：圆环仪表网格 —— CPU / 内存 / 磁盘 / 网络 / GPU，
//! 每类独立半透明色卡；下方统计行与接口状态。指标经 Signal 定向刷新（NFR-PERF-01）。

use windui::prelude::*;

use super::gauge::{gauge_card, Gauge};
use super::{kit, UiCtx};

pub fn page(ui: &UiCtx) -> Element {
    // ---- 仪表网格：2×2 大卡 + 统计侧栏（GPU 说明卡） ----
    let gauges = Element::grid(
        3,
        14,
        vec![
            gauge_card(ui.g_cpu, ui.t.d_cpu, ui.sub_cpu, kit::colors::GREEN, 36),
            gauge_card(ui.g_mem, ui.t.d_mem, ui.sub_mem, kit::colors::BLUE, 36),
            gauge_card(ui.g_disk, ui.t.d_disk, ui.sub_disk, kit::colors::ORANGE, 36),
            gauge_card(ui.g_net, ui.t.d_net, ui.sub_net, kit::colors::RED, 36),
            gpu_card(ui),
            stats_card(ui),
        ],
    );

    let hint = Element::label_signal(ui.t.d_hint)
        .fg_role(Role::TextMuted)
        .font_size(12.0);

    Element::scroll().child(
        Element::col()
            .fill()
            .padding(16)
            .spacing(14)
            .child(gauges)
            .child(hint),
    )
}

/// GPU 卡（任务 3）：macOS 经 ioreg 解析真实使用率；无数据时副文本显示 N/A。
fn gpu_card(ui: &UiCtx) -> Element {
    let card_bg = windui::geometry::Color::rgba(0x94, 0x6B, 0xD0, 30);
    Element::col()
        .fill()
        .padding(10)
        .bg(card_bg)
        .corner(14.0)
        .child(
            Element::leaf()
                .widget(Gauge::new(
                    ui.g_gpu,
                    ui.t.d_gpu,
                    ui.sub_gpu,
                    kit::colors::RED,
                ))
                .reactive(),
        )
}

/// 统计卡：进程/服务/接口状态（FR-DASH-01）。
fn stats_card(ui: &UiCtx) -> Element {
    let card_bg = windui::geometry::Color::rgba(0x3E, 0x6E, 0xB0, 30);
    Element::col()
        .fill()
        .padding(14)
        .bg(card_bg)
        .corner(14.0)
        .spacing(8)
        .child(
            Element::col()
                .spacing(2)
                .child(
                    Element::label_signal(ui.t.d_procs)
                        .fg_role(Role::TextMuted)
                        .font_size(11.5),
                )
                .child(
                    Element::label_signal(ui.d_total)
                        .font_size(16.0)
                        .font_weight(600),
                ),
        )
        .child(
            Element::col()
                .spacing(2)
                .child(
                    Element::label_signal(ui.t.d_running)
                        .fg_role(Role::TextMuted)
                        .font_size(11.5),
                )
                .child(
                    Element::label_signal(ui.d_running)
                        .font_size(16.0)
                        .font_weight(600)
                        .fg(kit::colors::GREEN),
                ),
        )
        .child(
            Element::col()
                .spacing(2)
                .child(
                    Element::label_signal(ui.t.d_svcs)
                        .fg_role(Role::TextMuted)
                        .font_size(11.5),
                )
                .child(
                    Element::label_signal(ui.d_services)
                        .font_size(16.0)
                        .font_weight(600)
                        .fg(kit::colors::BLUE),
                ),
        )
        .child(Element::divider())
        .child(
            Element::col()
                .spacing(2)
                .child(
                    Element::label_signal(ui.t.d_api)
                        .fg_role(Role::TextMuted)
                        .font_size(11.5),
                )
                .child(
                    Element::label_signal(ui.d_api)
                        .font_size(12.5)
                        .fg(kit::colors::GREEN),
                ),
        )
}
