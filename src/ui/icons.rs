//! 导航页签的 SVG 图标（内嵌常量，DPI 感知光栅化，tint 跟随主题）。
//! 单色 24×24 简笔线条：`stroke=currentColor` 由 windui 模板着色按 Role 上色。

/// 图标构造：SVG 字节 → DPI 感知 ImageContent（着色跟随主题 Text 角色）。
pub fn svg_icon(svg: &str) -> windui::ui::ImageContent {
    let mut c = windui::ui::ImageContent::from_svg_bytes(svg.as_bytes(), None);
    c.set_tint_role(windui::style::Role::Text);
    c
}

pub const DASHBOARD: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M4 13a8 8 0 0 1 16 0"/><path d="M12 13l3.5-3.5"/><circle cx="12" cy="13" r="1.4" fill="currentColor" stroke="none"/><path d="M3 17h18M5 20h14" opacity="0.6"/></svg>"#;

pub const PROCESSES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="7" rx="2"/><rect x="3" y="13" width="18" height="7" rx="2"/><path d="M7 7.5h.01M7 16.5h.01"/></svg>"#;

pub const SERVICES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="2.2"/><ellipse cx="12" cy="12" rx="9" ry="4"/><ellipse cx="12" cy="12" rx="9" ry="4" transform="rotate(60 12 12)"/><ellipse cx="12" cy="12" rx="9" ry="4" transform="rotate(120 12 12)"/></svg>"#;

pub const LOGS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3h11l4 4v14H5z"/><path d="M16 3v4h4"/><path d="M8.5 12h7M8.5 16h5"/></svg>"#;

pub const API: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 5l-5 7 5 7"/><path d="M16 5l5 7-5 7"/><path d="M13.5 4l-3 16"/></svg>"#;

pub const DOCS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M9.4 15.5a3.4 3.4 0 0 0 5.2 0"/><path d="M9 10h.01M15 10h.01"/></svg>"#;

pub const HELP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 9a2.6 2.6 0 1 1 3.8 2.3c-.8.5-1.3 1-1.3 2"/><path d="M12 16.8h.01"/><rect x="3" y="3" width="18" height="18" rx="4.5"/></svg>"#;

pub const SETTINGS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 2.8v3M12 18.2v3M4.2 6.5l2.6 1.5M17.2 16l2.6 1.5M2.8 12h3M18.2 12h3M4.2 17.5l2.6-1.5M17.2 8l2.6-1.5"/></svg>"#;
