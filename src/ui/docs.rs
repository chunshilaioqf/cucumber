//! 使用说明页（FR-DOC-01/02）：内嵌 Markdown 渲染（轻量转换，无重量级依赖）。

use windui::prelude::*;

use super::kit;
use crate::docs_embed;

pub fn page(_ui: &crate::ui::UiCtx) -> Element {
    let usage = Element::card(
        "添加与使用",
        Element::rich(kit::markdown_doc(docs_embed::USAGE)),
    );
    let ai = Element::card(
        "AI 接入",
        Element::rich(kit::markdown_doc(docs_embed::AI_GUIDE)),
    );
    Element::scroll().child(
        Element::col()
            .fill()
            .padding(16)
            .spacing(14)
            .child(usage)
            .child(ai),
    )
}
