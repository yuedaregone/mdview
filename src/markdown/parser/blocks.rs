//! 块级节点转换
//!
//! 将 comrak 的块级 AST 节点转换为我们的 DocNode 类型

use comrak::nodes::{AstNode, ListType, NodeValue};

use super::inlines::convert_inlines;
use super::tables::convert_table;
use super::types::*;

/// 转换子节点
pub fn convert_children<'a>(node: &'a AstNode<'a>) -> Vec<DocNode> {
    node.children()
        .filter_map(convert_node_with_table)
        .collect()
}

/// 转换单个节点（带表格支持）
fn convert_node_with_table<'a>(node: &'a AstNode<'a>) -> Option<DocNode> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Table(table) => convert_table(node, table),
        _ => convert_node(node),
    }
}

/// 转换单个块级节点
fn convert_node<'a>(node: &'a AstNode<'a>) -> Option<DocNode> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Document => None,
        NodeValue::FrontMatter(_) => None,

        NodeValue::Heading(heading) => {
            let children = convert_inlines(node);
            Some(DocNode::Heading {
                level: heading.level,
                children,
            })
        }

        NodeValue::Paragraph => {
            let children = convert_inlines(node);
            Some(DocNode::Paragraph(children))
        }

        NodeValue::CodeBlock(code_node) => {
            let lang = code_node.info.clone();
            let code = code_node.literal.clone();
            Some(DocNode::CodeBlock { lang, code })
        }

        NodeValue::List(list_node) => {
            let is_task = node.children().any(|child| {
                let cdata = child.data.borrow();
                matches!(&cdata.value, NodeValue::TaskItem(_))
            });

            if is_task {
                let items: Vec<TaskItem> = node
                    .children()
                    .filter_map(|child| {
                        let cdata = child.data.borrow();
                        match &cdata.value {
                            NodeValue::TaskItem(checked) => Some(TaskItem {
                                checked: checked.is_some(),
                                children: convert_children(child),
                            }),
                            NodeValue::Item(_) => Some(TaskItem {
                                checked: false,
                                children: convert_children(child),
                            }),
                            _ => None,
                        }
                    })
                    .collect();
                Some(DocNode::TaskList { items })
            } else {
                let items: Vec<ListItem> = node
                    .children()
                    .filter(|child| matches!(&child.data.borrow().value, NodeValue::Item(_)))
                    .map(|child| ListItem {
                        children: convert_children(child),
                    })
                    .collect();

                match list_node.list_type {
                    ListType::Ordered => Some(DocNode::OrderedList {
                        start: list_node.start as u64,
                        items,
                    }),
                    ListType::Bullet => Some(DocNode::UnorderedList(items)),
                }
            }
        }

        NodeValue::BlockQuote => {
            let children = convert_children(node);
            Some(DocNode::BlockQuote(children))
        }

        NodeValue::ThematicBreak => Some(DocNode::ThematicBreak),

        NodeValue::Image(_image_node) => {
            let alt = comrak::nodes::AstNode::children(node)
                .map(collect_text_for_image)
                .collect::<Vec<_>>()
                .join("");

            if alt.is_empty() {
                None
            } else {
                Some(DocNode::Paragraph(vec![InlineNode::Text(alt)]))
            }
        }

        NodeValue::HtmlBlock(html_node) => Some(DocNode::HtmlBlock(html_node.literal.clone())),

        NodeValue::FootnoteDefinition(fn_def) => {
            let label = fn_def.name.clone();
            let content = convert_children(node);
            Some(DocNode::FootnoteDef { label, content })
        }

        NodeValue::Item(_) | NodeValue::TaskItem(_) => None,
        _ => None,
    }
}

/// 收集节点中的纯文本
fn collect_text_for_image<'a>(node: &'a AstNode<'a>) -> String {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(t) => t.clone(),
        _ => node
            .children()
            .map(collect_text_for_image)
            .collect::<Vec<_>>()
            .join(""),
    }
}
