//! 设置页（任务 3）：主题（浅色/深色/跟随系统）、字体与字号缩放、
//! 窗口背景色（含透明度）、系统托盘开关、九语言切换。

use windui::prelude::*;

use super::i18n::{self, Lang};
use super::UiCtx;

/// 常见跨平台字体建议（占位提示；实际以系统已装字体为准）。
pub const PLATFORM_FONTS: [&str; 4] = [
    "Microsoft YaHei UI",
    "PingFang SC",
    "Noto Sans CJK SC",
    "monospace",
];

/// 应用主题：模式 × 字体 × 缩放 × 背景覆盖（设置页与启动时共用）。
pub fn apply_theme(ui: &UiCtx) {
    let mode = ui.theme_mode.get();
    let family = ui.font_family.get();
    let scale = ui.font_scale.get().clamp(0.8, 1.5);
    let (bg_use, bg) = (ui.bg_use.get(), ui.bg_color.get());
    ui.theme.update(move |t| {
        match mode {
            0 => *t = Theme::default(),
            1 => *t = Theme::dark(),
            _ => {} // 跟随系统：由 on_system_theme_changed 驱动
        }
        // 字体族
        t.font_family = if family.is_empty() {
            None
        } else {
            Some(family.clone())
        };
        // 字号整体缩放（保持层级差）
        t.metrics.font_xs = (12.0 * scale).max(9.0);
        t.metrics.font_sm = (13.0 * scale).max(10.0);
        t.metrics.font_md = (14.0 * scale).max(10.0);
        t.metrics.font_lg = (16.0 * scale).max(11.0);
        t.metrics.font_xl = (20.0 * scale).max(12.0);
        // 背景覆盖（含 alpha；作用于应用内绘制层）
        if bg_use {
            t.palette.bg = bg;
        }
    });
}

/// 立即持久化全部设置。
pub fn persist(ui: &UiCtx) {
    if let Ok(mut c) = ui.app.config.write() {
        c.general.theme_mode = match ui.theme_mode.get() {
            0 => "light".into(),
            1 => "dark".into(),
            _ => "system".into(),
        };
        let f = ui.font_family.get();
        c.general.font_family = if f.is_empty() { None } else { Some(f) };
        c.general.font_scale = ui.font_scale.get();
        c.general.bg_color = if ui.bg_use.get() {
            Some(super::to_hex_rgba(ui.bg_color.get()))
        } else {
            None
        };
        c.general.tray_enabled = ui.tray_enabled.get();
    }
    let _ = ui.app.save_config();
}

pub fn page(ui_ref: &UiCtx) -> Element {
    let ui = ui_ref.clone();
    let f = |key: &str| i18n::tr(ui.lang.get(), key);

    // ---- 主题 ----
    let theme_row = Element::field(
        f("st.theme"),
        Element::segmented(
            vec![f("st.light"), f("st.dark"), f("st.system")],
            ui.theme_mode,
        )
        .on_segment_change({
            let ui = ui.clone();
            move |_ctx, _i| {
                apply_theme(&ui);
                persist(&ui);
            }
        })
        .width(330),
    );

    // ---- 字体（自由输入 + 常用字体下拉建议）----
    let font_input = Element::field(
        f("st.font"),
        Element::hstack()
            .spacing(8)
            .child(
                Element::text_input(ui.font_family, PLATFORM_FONTS.join(" / "))
                    .width(300)
                    .on_commit({
                        let ui = ui.clone();
                        move |_ctx| {
                            apply_theme(&ui);
                            persist(&ui);
                        }
                    }),
            )
            .child(
                Element::button(f("st.fontdef"))
                    .small()
                    .neutral()
                    .on_click({
                        let ui = ui.clone();
                        move |_| {
                            ui.font_family.set(String::new());
                            apply_theme(&ui);
                            persist(&ui);
                        }
                    })
                    .tooltip("Microsoft YaHei UI / PingFang SC / monospace"),
            ),
    );

    // ---- 字号缩放（slider 无回调，经 1s tick 桥接应用）----
    let scale_row = Element::setting_row(
        f("st.fontsize"),
        Element::hstack()
            .spacing(10)
            .child(Element::slider(ui.font_norm).width(230))
            .child(Element::label_signal(ui.font_pct).width(56)),
    );

    // ---- 背景色（含透明度）----
    let bg_row = Element::field(
        f("st.bg"),
        Element::hstack()
            .spacing(10)
            .child(Element::switch(ui.bg_use).on_toggle({
                let ui = ui.clone();
                let sig = ui.bg_use;
                move |_ctx| {
                    sig.set(!sig.get());
                    apply_theme(&ui);
                    persist(&ui);
                }
            }))
            .child(Element::color_picker_opts(
                ui.bg_color,
                ColorPickerOpts::default().alpha(true).trigger_text(false),
            ))
            .child(Element::button(f("c.save")).small().on_click({
                let ui = ui.clone();
                move |_| {
                    apply_theme(&ui);
                    persist(&ui);
                }
            }))
            .child(
                Element::label(f("st.bghint"))
                    .fg_role(Role::TextMuted)
                    .font_size(11.5),
            ),
    );

    // ---- 系统托盘（重启生效）----
    let tray_row = Element::setting_row_desc(
        f("st.tray"),
        f("st.trayhint"),
        Element::switch(ui.tray_enabled).on_toggle({
            let ui = ui.clone();
            let sig = ui.tray_enabled;
            move |_ctx| {
                sig.set(!sig.get());
                persist(&ui);
            }
        }),
    );

    // ---- 语言 ----
    let lang_row = Element::field(
        f("st.lang"),
        Element::dropdown_signal(
            signal(Lang::ALL.iter().map(|l| l.native().to_string()).collect()),
            ui.lang_sel,
        )
        .width(230),
    );

    let card = Element::card(
        f("st.appearance"),
        Element::col()
            .width_match()
            .spacing(10)
            .child(theme_row)
            .child(font_input)
            .child(scale_row)
            .child(bg_row)
            .child(tray_row)
            .child(lang_row)
            .child(
                Element::label(f("st.note"))
                    .fg_role(Role::TextMuted)
                    .font_size(11.5),
            ),
    );

    Element::scroll().child(Element::col().fill().padding(16).spacing(12).child(card))
}
