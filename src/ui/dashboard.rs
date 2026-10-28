//! 仪表盘（FR-DASH-01~03）：只读概览，指标经 Signal 定向刷新（NFR-PERF-01）。

use windui::prelude::*;

use super::kit;
use super::UiCtx;

pub fn page(ui: &UiCtx) -> Element {
    let cards = Element::grid(
        3,
        12,
        vec![
            kit::stat_card("已配置进程总数", ui.d_total, None),
            kit::stat_card("运行中进程数", ui.d_running, Some(kit::colors::GREEN)),
            kit::stat_card("已配置服务总数", ui.d_services, Some(kit::colors::BLUE)),
            kit::stat_card("本机 CPU 使用率", ui.d_cpu, Some(kit::colors::ORANGE)),
            kit::stat_card("本机内存使用率", ui.d_mem, Some(kit::colors::ORANGE)),
            kit::stat_card("对外接口监听状态", ui.d_api, Some(kit::colors::GREEN)),
        ],
    );

    let hint = Element::label("仪表盘为只读概览；进程操作入口在「进程」页（FR-DASH-03）")
        .fg_role(Role::TextMuted)
        .font_size(12.0);

    Element::scroll().child(
        Element::col()
            .fill()
            .padding(16)
            .spacing(12)
            .child(cards)
            .child(hint),
    )
}
