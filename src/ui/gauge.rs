//! 仪表盘控件（任务 5）：圆环仪表自绘 Widget。
//!
//! 半透明色卡由外层容器（dashboard）提供；本控件画弧环（背景环 + 值弧）、
//! 中心大号百分比、标题与副文本（如“3.2 GB / 16 GB”）。绑定 `Signal`，
//! 版本变化时经 `on_update` 请求重绘（`Element::reactive` 机制）。

use std::cell::Cell;

use windui::geometry::{Color, Rect, Size};
use windui::prelude::*;
use windui::render::Paint;

/// 圆环仪表：value ∈ 0..=1；label 为标题信号；sub 为副文本信号。
pub struct Gauge {
    value: Signal<f32>,
    label: Signal<String>,
    sub: Signal<String>,
    accent: Color,
    last_ver: Cell<(u64, u64, u64)>,
}

impl Gauge {
    pub fn new(
        value: Signal<f32>,
        label: Signal<String>,
        sub: Signal<String>,
        accent: Color,
    ) -> Self {
        Self {
            value,
            label,
            sub,
            accent,
            last_ver: Cell::new((0, 0, 0)),
        }
    }
}

impl windui::core::Widget for Gauge {
    fn measure(
        &self,
        _avail: Size,
        _style: &Style,
        _text: &mut dyn windui::text::TextEngine,
    ) -> Size {
        Size::new(180, 170)
    }

    fn on_update(&mut self, _ctx: &mut windui::core::EventCtx) {
        let v = (
            self.value.version(),
            self.label.version(),
            self.sub.version(),
        );
        if v != self.last_ver.get() {
            self.last_ver.set(v);
            _ctx.mark_dirty();
        }
    }

    fn paint(
        &self,
        bounds: Rect,
        _content: Rect,
        _focused: bool,
        _enabled: bool,
        canvas: &mut dyn windui::render::Canvas,
        _style: &Style,
    ) {
        let frac = self.value.get().clamp(0.0, 1.0);
        let label = self.label.get();
        let sub = self.sub.get();
        let th = windui::theme::current();
        let text_col = th.palette.text;
        let muted = th.palette.text_muted;

        let cx = bounds.x as f32 + bounds.w as f32 / 2.0;
        // 中心（略上移给副文本留位）
        let cy = bounds.y as f32 + bounds.h as f32 / 2.0 + 6.0;
        let r_out = (bounds.h as f32 * 0.30).min(bounds.w as f32 * 0.34);
        let r_in = r_out * 0.74;

        // 仪表弧：240° 扇区（-210° .. +30°，0=右侧，y 向下）
        let a0 = 150.0_f32.to_radians(); // 左下起点
        let a1 = 390.0_f32.to_radians(); // 右下终点（跨过顶部）
        let arc = |a: f32| {
            (
                cx + a.cos() * (r_out + r_in) * 0.5,
                cy + a.sin() * (r_out + r_in) * 0.5,
            )
        };
        let ring_w = (r_out - r_in).max(6.0);

        // 背景环（整段，主题灰半透明）
        let bg_paint = Paint::fill(Color::rgba(text_col.r, text_col.g, text_col.b, 38));
        stroke_arc(canvas, a0, a1, arc, ring_w, bg_paint);
        // 值弧（accent 渐变近似：主色 + 起点亮色）
        let val_paint = Paint::fill(self.accent);
        if frac > 0.004 {
            stroke_arc(canvas, a0, a0 + (a1 - a0) * frac, arc, ring_w, val_paint);
        }

        // 中心文字：百分比大字
        let mut ts = crate::ui::gauge::text_style(th.font_family.as_deref(), 21.0, 600);
        let pct = format!("{}%", (frac * 100.0).round() as i32);
        let rect = Rect::new((cx - 60.0) as i32, (cy - 16.0) as i32, 120, 24);
        canvas.draw_text(&pct, rect, th.palette.text, Align::Center, &ts);
        // 标题（环上方）
        ts = crate::ui::gauge::text_style(th.font_family.as_deref(), 12.5, 500);
        let lrect = Rect::new((cx - 90.0) as i32, bounds.y + 8, 180, 18);
        canvas.draw_text(&label, lrect, muted, Align::Center, &ts);
        // 副文本（环内下方）
        ts = crate::ui::gauge::text_style(th.font_family.as_deref(), 11.0, 400);
        let srect = Rect::new((cx - 90.0) as i32, (cy + r_out * 0.45) as i32, 180, 16);
        canvas.draw_text(&sub, srect, muted, Align::Center, &ts);
    }
}

fn text_style(family: Option<&str>, size: f32, weight: u16) -> windui::text::TextStyle<'static> {
    let mut ts = windui::text::TextStyle::new(size);
    ts.weight = weight;
    if let Some(f) = family {
        ts.family = Some(cached_family(f));
    }
    ts
}

/// 每个字族名只泄漏一次，避免 paint 每帧泄漏。
fn cached_family(f: &str) -> &'static str {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static CACHE: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
    }
    CACHE.with(|c| {
        let mut m = c.borrow_mut();
        if let Some(s) = m.get(f) {
            return *s;
        }
        let leaked: &'static str = Box::leak(f.to_string().into_boxed_str());
        m.insert(f.to_string(), leaked);
        leaked
    })
}

/// 用粗线段+圆帽近似弧（每 3° 一段，两端圆帽）。
fn stroke_arc(
    canvas: &mut dyn windui::render::Canvas,
    a0: f32,
    a1: f32,
    arc: impl Fn(f32) -> (f32, f32),
    width: f32,
    paint: Paint,
) {
    const STEP: f32 = 3.0_f32.to_radians();
    let n = ((a1 - a0) / STEP).ceil().max(1.0) as usize;
    let mut prev = arc(a0);
    canvas.fill_circle(prev.0, prev.1, width / 2.0, &paint);
    for i in 1..=n {
        let a = a0 + (a1 - a0) * (i as f32 / n as f32);
        let p = arc(a);
        canvas.draw_line(prev.0, prev.1, p.0, p.1, width, &paint);
        prev = p;
    }
    canvas.fill_circle(prev.0, prev.1, width / 2.0, &paint);
}

/// 仪表卡片：半透明色卡背景 + 仪表。
pub fn gauge_card(
    value: Signal<f32>,
    label: Signal<String>,
    sub: Signal<String>,
    accent: Color,
    alpha: u8,
) -> Element {
    let card_bg = Color::rgba(accent.r, accent.g, accent.b, alpha);
    Element::col()
        .fill()
        .padding(10)
        .bg(card_bg)
        .corner(14.0)
        .child(
            Element::leaf()
                .widget(Gauge::new(value, label, sub, accent))
                .reactive(),
        )
}
