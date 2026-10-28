//! UI 适配层（NFR-MAINT-03 / ADR-07）：集中封装 windui 的控件构建调用。
//! windui API 变动时，业务页面的改动面收口在本文件与各页面的骨架组装处。

use windui::prelude::*;

/// 主题语义色（供日志级别着色等使用，避免硬编码遍布页面）。
pub mod colors {
    use windui::geometry::Color;
    pub const RED: Color = Color::rgb(0xC4, 0x3E, 0x3E);
    pub const ORANGE: Color = Color::rgb(0xB8, 0x82, 0x30);
    pub const GREEN: Color = Color::rgb(0x3E, 0x8E, 0x51);
    pub const BLUE: Color = Color::rgb(0x3E, 0x6E, 0xB0);
    pub const GRAY: Color = Color::rgb(0x88, 0x88, 0x88);
    pub const YELLOW_BG: Color = Color::rgba(0xF7, 0xD1, 0x59, 0xB0);
}

/// 轻量 Markdown → RichDoc（FR-DOC-01：文档内嵌、无重量级渲染依赖）。
/// 支持：# 标题、列表（- / *）、``` 代码块、表格（按行等宽显示）、行内 `code`。
pub fn markdown_doc(md: &str) -> RichDoc {
    let mut doc = RichDoc::new()
        .style("h1", SpanStyle::new().size(17.0).bold())
        .style("h2", SpanStyle::new().size(15.0).bold())
        .style("h3", SpanStyle::new().size(13.5).bold())
        .style(
            "code",
            SpanStyle::new()
                .family("monospace")
                .size(12.0)
                .fg(colors::BLUE),
        )
        .style("quote", SpanStyle::new().fg(colors::GRAY).italic());
    let mut in_code = false;
    for line in md.lines() {
        let t = line.trim_end();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            doc = doc.para(Para::new().span(t, SpanStyle::new().family("monospace").size(12.0)));
            continue;
        }
        if t.is_empty() {
            continue;
        }
        let para = if let Some(rest) = t.strip_prefix("### ") {
            Para::new().styled("h3", rest)
        } else if let Some(rest) = t.strip_prefix("## ") {
            Para::new().styled("h2", rest)
        } else if let Some(rest) = t.strip_prefix("# ") {
            Para::new().styled("h1", rest)
        } else if t.starts_with("- ") || t.starts_with("* ") {
            Para::new()
                .span("· ", SpanStyle::new().fg(colors::GRAY))
                .span(&t[2..], SpanStyle::new())
        } else if let Some(rest) = t.strip_prefix("> ") {
            Para::new().styled("quote", rest)
        } else if t.starts_with('|') {
            Para::new().span(t, SpanStyle::new().family("monospace").size(12.0))
        } else if t.starts_with("    ") || t.starts_with('\t') {
            Para::new().span(t.trim(), SpanStyle::new().family("monospace").size(12.0))
        } else {
            // 行内 `code` 片段
            let mut para = Para::new();
            let mut rest = t;
            let mut has = false;
            while let Some(pos) = rest.find('`') {
                if let Some(end) = rest[pos + 1..].find('`') {
                    if pos > 0 {
                        para = para.span(&rest[..pos], SpanStyle::new());
                    }
                    para = para.styled("code", &rest[pos + 1..pos + 1 + end]);
                    rest = &rest[pos + 2 + end..];
                    has = true;
                } else {
                    break;
                }
            }
            if has {
                para = para.span(rest, SpanStyle::new())
            } else {
                para = para.span(t, SpanStyle::new())
            }
            para
        };
        doc = doc.para(para);
    }
    doc
}
