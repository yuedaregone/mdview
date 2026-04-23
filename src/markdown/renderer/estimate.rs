//! 块级元素高度估算

use crate::markdown::parser::{DocNode, InlineNode, ListItem};
use crate::theme::Theme;
use crate::viewport::DEFAULT_BLOCK_HEIGHT;

/// 估算块级元素高度
pub fn estimate_block_height(node: &DocNode, theme: &Theme, font_size: f32) -> f32 {
    match node {
        DocNode::Heading { level, children } => {
            let text_len = inline_text_len(children) as f32;
            let approx_lines = estimate_line_count(text_len, 48.0);
            theme.heading_size(*level, font_size) * approx_lines * 1.2 + 20.0
        }
        DocNode::Paragraph(inlines) => {
            let text_len = inline_text_len(inlines) as f32;
            font_size * 1.55 * estimate_line_count(text_len, 72.0) + 12.0
        }
        DocNode::CodeBlock { lang, code } => {
            let lines = code.lines().count().max(1).min(32) as f32;
            let label_height = if lang.is_empty() {
                0.0
            } else {
                font_size * 0.8 + 8.0
            };
            lines * font_size * 1.2 + label_height + 40.0
        }
        DocNode::Table { rows, .. } => {
            let row_count = rows.len() as f32 + 1.0;
            row_count * (font_size * 1.8) + 24.0
        }
        DocNode::BlockQuote(children) => {
            children
                .iter()
                .map(|child| estimate_block_height(child, theme, font_size * 0.95))
                .sum::<f32>()
                + 12.0
        }
        DocNode::OrderedList { items, .. } | DocNode::UnorderedList(items) => {
            estimate_list_height(items, theme, font_size)
        }
        DocNode::TaskList { items } => items
            .iter()
            .map(|item| {
                item.children
                    .iter()
                    .map(|child| estimate_block_height(child, theme, font_size))
                    .sum::<f32>()
                    + 8.0
            })
            .sum::<f32>()
            .max(font_size * 1.5 + 8.0),
        DocNode::ThematicBreak => 24.0,
        DocNode::Image { .. } => 220.0,
        DocNode::HtmlBlock(html) => {
            let lines = html.lines().count().max(1).min(24) as f32;
            lines * font_size * 1.2 + 24.0
        }
        DocNode::FootnoteDef { content, .. } => {
            content
                .iter()
                .map(|child| estimate_block_height(child, theme, font_size))
                .sum::<f32>()
                + font_size * 1.4
        }
    }
    .max(DEFAULT_BLOCK_HEIGHT * 0.5)
}

/// 估算列表高度
pub fn estimate_list_height(items: &[ListItem], theme: &Theme, font_size: f32) -> f32 {
    items
        .iter()
        .map(|item| {
            item.children
                .iter()
                .map(|child| estimate_block_height(child, theme, font_size))
                .sum::<f32>()
                + 8.0
        })
        .sum::<f32>()
        .max(font_size * 1.5 + 8.0)
}

/// 估算内联文本长度（字符数）
fn inline_text_len(inlines: &[InlineNode]) -> usize {
    inlines.iter().map(inline_len).sum()
}

/// 单个内联元素的长度
fn inline_len(inline: &InlineNode) -> usize {
    match inline {
        InlineNode::Text(s)
        | InlineNode::Code(s)
        | InlineNode::Superscript(s)
        | InlineNode::HtmlInline(s) => s.chars().count(),
        InlineNode::Bold(children)
        | InlineNode::Italic(children)
        | InlineNode::Strikethrough(children) => inline_text_len(children),
        InlineNode::Link { children, .. } => inline_text_len(children),
        InlineNode::Image { alt, .. } => alt.chars().count().max(8),
        InlineNode::SoftBreak | InlineNode::HardBreak => 1,
        InlineNode::FootnoteRef(label) => label.chars().count() + 3,
    }
}

/// 估算行数
fn estimate_line_count(text_len: f32, chars_per_line: f32) -> f32 {
    (text_len / chars_per_line).ceil().clamp(1.0, 12.0)
}
