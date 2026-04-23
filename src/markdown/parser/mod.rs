//! Markdown 解析器：comrak AST → 简化的 MarkdownDoc
//!
//! 将 comrak 的 Arena-based AST 转换为简单的所有权树

mod blocks;
mod inlines;
mod tables;
pub mod types;

pub use types::*;

use comrak::{parse_document, Arena, Options};

/// 构建 comrak 选项
fn build_options() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts.extension.superscript = true;
    opts.extension.footnotes = true;
    opts.extension.description_lists = false;
    opts.extension.front_matter_delimiter = Some("---".to_string());
    opts.parse.smart = true;
    opts.parse.default_info_string = Some("text".to_string());
    opts
}

/// 完整解析 markdown 文本
pub fn parse_full(text: &str) -> MarkdownDoc {
    let arena = Arena::new();
    let options = build_options();
    let root = parse_document(&arena, text, &options);
    let nodes = blocks::convert_children(root);
    MarkdownDoc { nodes }
}
