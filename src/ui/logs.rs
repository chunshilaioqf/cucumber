//! 日志页（FR-LOG-01~07，R-02）：过滤检索、分页、级别着色与关键词高亮。
//!
//! 查询在 `spawn_blocking` 中执行（NFR-PERF-02：Rust 侧过滤，只回传当前页）；
//! 高亮区间在数据层计算后构造 RichDoc span 交给 `rich` 渲染。

use windui::prelude::*;

use super::kit;
use super::{UiCtx, UiLogLine, UiMsg};
use crate::core::config::LogSource;
use crate::core::log_index::{highlight_ranges, LogFilter, LogIndex};
use crate::core::log_parser::{format_ts, Level, LogLine};
use crate::ui::LogBackend;

pub fn page(ui: &UiCtx) -> Element {
    // ---- 过滤栏 ----
    let bar = Element::col()
        .width_match()
        .spacing(8)
        .child(
            Element::hstack()
                .width_match()
                .spacing(10)
                .child(Element::dropdown_signal(ui.log_proc_names, ui.log_proc).width(150))
                .child(Element::dropdown_signal(ui.log_src_names, ui.log_src).width(120))
                .child(Element::text_input(ui.log_q, "搜索消息体（子串）").width(220))
                .child(Element::text_input(ui.log_tag, "标签（子串）").width(120)),
        )
        .child(
            Element::hstack()
                .width_match()
                .spacing(10)
                .child(Element::label("级别：").fg_role(Role::TextMuted))
                .child(Element::checkbox("ERROR", ui.log_levels[0]))
                .child(Element::checkbox("WARN", ui.log_levels[1]))
                .child(Element::checkbox("INFO", ui.log_levels[2]))
                .child(Element::checkbox("DEBUG", ui.log_levels[3]))
                .child(Element::checkbox("TRACE", ui.log_levels[4]))
                .child(Element::flex_spacer())
                .child(Element::label("跟随").fg_role(Role::TextMuted))
                .child(Element::switch(ui.log_follow))
                .child(Element::button("查询").on_click({
                    let ui = ui.clone();
                    move |_| {
                        ui.log_page.set(0);
                        spawn_query(&ui, 0);
                    }
                })),
        );

    // ---- 分页条 ----
    let pager = Element::hstack()
        .width_match()
        .spacing(8)
        .child(Element::icon_button("‹").tooltip("上一页").on_click({
            let ui = ui.clone();
            move |_| {
                let p = ui.log_page.get().saturating_sub(1);
                ui.log_page.set(p);
                spawn_query(&ui, p);
            }
        }))
        .child(Element::icon_button("›").tooltip("下一页").on_click({
            let ui = ui.clone();
            move |_| {
                let p = ui.log_page.get() + 1;
                ui.log_page.set(p);
                spawn_query(&ui, p);
            }
        }))
        .child(
            Element::label_signal(ui.log_info)
                .fg_role(Role::TextMuted)
                .font_size(12.0),
        );

    // ---- 结果列表 ----
    let q_sig = ui.log_q;
    let list = Element::list_signal(
        ui.log_lines,
        |r: &UiLogLine| r.no,
        move |r: UiLogLine| render_line(&r.line, r.no, &q_sig.get()),
    )
    .weight(1.0);

    let empty = {
        let lines = ui.log_lines;
        Element::label("无匹配行。调整过滤条件后点击「查询」。")
            .fg_role(Role::TextMuted)
            .visible_when(move || lines.with(|r| r.is_empty()))
    };

    Element::col()
        .fill()
        .padding_xy(16, 8)
        .spacing(8)
        .child(bar)
        .child(pager)
        .child(empty)
        .child(list)
}

fn level_color(l: Level) -> windui::geometry::Color {
    match l {
        Level::Error => kit::colors::RED,
        Level::Warn => kit::colors::ORANGE,
        Level::Info => kit::colors::GREEN,
        Level::Debug => kit::colors::BLUE,
        Level::Trace => kit::colors::GRAY,
    }
}

/// 渲染一行：行号 + 时间戳 + 级别 + 标签 + 消息（关键词黄底高亮，R-02 第一步）。
fn render_line(line: &LogLine, no: usize, q: &str) -> Element {
    let mono = |c: Option<windui::geometry::Color>| {
        let mut st = SpanStyle::new().family("monospace").size(12.0);
        if let Some(c) = c {
            st = st.fg(c);
        }
        st
    };
    let mut para = Para::new().span(format!("{no:>7} "), mono(Some(kit::colors::GRAY)));
    if let Some(ts) = line.ts {
        para = para.span(format!("{} ", format_ts(ts)), mono(Some(kit::colors::GRAY)));
    }
    if let Some(l) = line.level {
        para = para.span(
            format!("{:<5} ", l.label()),
            mono(Some(level_color(l))).bold(),
        );
    }
    for t in &line.tags {
        para = para.span(format!("[{t}] "), mono(Some(kit::colors::BLUE)));
    }
    // 关键词高亮：命中片段黄底
    let msg = &line.message;
    let ranges = highlight_ranges(msg, q);
    if ranges.is_empty() {
        para = para.span(msg.clone(), mono(None));
    } else {
        let chars: Vec<char> = msg.chars().collect();
        let mut last = 0usize;
        for (s, e) in ranges
            .iter()
            .chain(std::iter::once(&(chars.len(), chars.len())))
        {
            if *s > last {
                let seg: String = chars[last..*s].iter().collect();
                para = para.span(seg, mono(None));
            }
            if *e > *s {
                let seg: String = chars[*s..*e].iter().collect();
                para = para.span(seg, mono(Some(kit::colors::YELLOW_BG)));
            }
            last = *e;
        }
    }
    Element::rich(RichDoc::new().para(para))
        .width_match()
        .padding_xy(8, 3)
}

/// 发起一次日志查询（阻塞索引与过滤在 spawn_blocking；结果经 channel 回 UI）。
/// `page` 为目标页码（0 基）；超大值会被钳制到最后一页。
pub fn spawn_query(ui: &UiCtx, page: usize) {
    let Some((_proc_id, src)) = ui.log_selection() else {
        let _ = ui.tx.send(UiMsg::ToastErr("请先为进程配置日志源".into()));
        return;
    };
    // 在 UI 线程收齐全部参数（Signal 读取），后台任务只携带 Send 数据
    let q = ui.log_q.get();
    let tag = ui.log_tag.get();
    let levels: Vec<String> = Level::all()
        .iter()
        .enumerate()
        .filter(|(i, _)| ui.log_levels[*i].get())
        .map(|(_, l)| l.label().to_string())
        .collect();
    let page_size = ui
        .app
        .config
        .read()
        .map(|c| c.general.log_page_size.clamp(50, 1000))
        .unwrap_or(100);
    let backend = ui.log_backend.clone();
    let tx = ui.tx.clone();
    let rt = ui.rt.clone();
    rt.spawn_blocking(move || {
        let src_key = src
            .path
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| src.command.clone())
            .unwrap_or_default();
        let outcome = run_query(&backend, &src, &src_key, &q, &tag, &levels, page, page_size);
        match outcome {
            Ok((lines, hits, page_no, info)) => {
                let _ = tx.send(UiMsg::LogReady(lines, hits, info, page_no));
            }
            Err(e) => {
                let _ = tx.send(UiMsg::ToastErr(e));
            }
        }
    });
}

#[allow(clippy::too_many_arguments)] // 查询参数天然多维，强拆结构体反而绕
fn run_query(
    backend: &std::sync::Arc<std::sync::Mutex<LogBackend>>,
    src: &LogSource,
    src_key: &str,
    q: &str,
    tag: &str,
    levels: &[String],
    page: usize,
    page_size: usize,
) -> Result<(Vec<UiLogLine>, usize, usize, String), String> {
    let mut be = backend.lock().map_err(|e| e.to_string())?;
    if be.src_key != src_key {
        be.index = Some(LogIndex::open(src)?);
        be.src_key = src_key.to_string();
    }
    let out = (|| {
        let idx = be.index.as_ref()?;
        let filter = LogFilter {
            levels: levels.iter().filter_map(|l| Level::parse(l)).collect(),
            tag: if tag.trim().is_empty() {
                None
            } else {
                Some(tag.trim().to_string())
            },
            ts_from: None,
            ts_to: None,
            query: if q.trim().is_empty() {
                None
            } else {
                Some(q.trim().to_string())
            },
        };
        let (hits, _tags) = idx.filter(&filter);
        let total_pages = hits.len().div_ceil(page_size).max(1);
        let page_no = page.min(total_pages - 1);
        let pg = idx.page(&hits, page_no, page_size);
        Some((pg, page_no))
    })();
    let Some((pg, page_no)) = out else {
        return Err("日志索引不可用".into());
    };
    let info = format!(
        "共 {} 行 · 命中 {} · 每页 {}  ",
        pg.total_lines, pg.total_hits, pg.page_size
    );
    let lines: Vec<UiLogLine> = pg
        .lines
        .into_iter()
        .map(|(no, line)| UiLogLine { no, line })
        .collect();
    Ok((lines, pg.total_hits, page_no, info))
}

/// 跟随模式 tick（FR-LOG-06）：main 的 on_interval 调用；开启时跳到最新一页。
/// v1 实现为全量重查（尾部增量索引留待后续版本，见 docs/架构设计.md §8）。
pub fn follow_tick(ui: &UiCtx) {
    if ui.log_follow.get() && ui.page.get() == 3 {
        // 超大页码在查询内被钳制到最后一页
        spawn_query(ui, usize::MAX / 2);
    }
}
