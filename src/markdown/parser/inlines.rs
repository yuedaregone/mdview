//! 内联节点转换
//!
//! 将 comrak 的内联 AST 节点转换为我们的 InlineNode 类型

use comrak::nodes::{AstNode, NodeValue};

use super::types::InlineNode;

/// 转换内联节点
pub fn convert_inlines<'a>(node: &'a AstNode<'a>) -> Vec<InlineNode> {
    node.children().filter_map(convert_inline).collect()
}

/// 转换单个内联节点
fn convert_inline<'a>(node: &'a AstNode<'a>) -> Option<InlineNode> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(text) => Some(InlineNode::Text(text.clone())),
        NodeValue::SoftBreak => Some(InlineNode::SoftBreak),
        NodeValue::LineBreak => Some(InlineNode::HardBreak),
        NodeValue::Emph => {
            let children = convert_inlines(node);
            Some(InlineNode::Italic(children))
        }
        NodeValue::Strong => {
            let children = convert_inlines(node);
            Some(InlineNode::Bold(children))
        }
        NodeValue::Strikethrough => {
            let children = convert_inlines(node);
            Some(InlineNode::Strikethrough(children))
        }
        NodeValue::Code(code_node) => Some(InlineNode::Code(code_node.literal.clone())),
        NodeValue::Link(link_node) => {
            let children = convert_inlines(node);
            Some(InlineNode::Link {
                url: link_node.url.clone(),
                children,
            })
        }
        NodeValue::Image(_image_node) => Some(InlineNode::Text(collect_text(node))),
        NodeValue::FootnoteReference(fn_ref) => Some(InlineNode::FootnoteRef(fn_ref.name.clone())),
        NodeValue::Superscript => {
            let text = collect_text(node);
            Some(InlineNode::Superscript(text))
        }
        NodeValue::HtmlInline(html) => Some(InlineNode::HtmlInline(html.clone())),
        NodeValue::Escaped => {
            let children = convert_inlines(node);
            Some(InlineNode::Text(
                children.iter().map(|n| n.plain_text()).collect(),
            ))
        }
        _ => None,
    }
}

/// 收集内联树中的所有纯文本
pub fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(t) => t.clone(),
        _ => node
            .children()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
    }
}
