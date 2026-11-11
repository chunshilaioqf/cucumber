//! 使用说明页（FR-DOC-01/02）：内嵌 Markdown 渲染（轻量转换，无重量级依赖）。

use windui::prelude::*;

use super::kit;
use crate::docs_embed;

pub fn page(ui: &crate::ui::UiCtx) -> Element {
    // 0.6.0：注明正文语言 + 语言切换指引（i18n）
    let note = Element::label_signal(ui.t.d2_note)
        .fg_role(Role::TextMuted)
        .font_size(12.0);
    let usage = Element::card(
        ui.t.d2_adduse.get(),
        Element::rich(kit::markdown_doc(docs_embed::USAGE)),
    );
    let ai = Element::card(
        ui.t.d2_ai.get(),
        Element::rich(kit::markdown_doc(docs_embed::AI_GUIDE)),
    );
    Element::scroll().child(
        Element::col()
            .fill()
            .padding(16)
            .spacing(14)
            .child(note)
            .child(usage)
            .child(ai),
    )
}
