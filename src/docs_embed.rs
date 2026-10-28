//! 内嵌文档（FR-DOC-02：Markdown 以 include_str! 编进二进制，不硬编码在 UI 代码）。

/// 「使用说明 · 添加与使用」
pub const USAGE: &str = include_str!("../docs/usage.md");
/// 「使用说明 · AI 接入」
pub const AI_GUIDE: &str = include_str!("../docs/ai-guide.md");
