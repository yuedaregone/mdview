//! Markdown parser: comrak AST → simplified MarkdownDoc
//!
//! Design: Convert comrak's Arena-based AST into a simple owned tree.
//! We only extract node types needed for reading (no edit operations).

#![allow(dead_code)]

use std::fmt;

use comrak::nodes::*;
use comrak::{parse_document, Arena, Options};

// ─── Public Types ───────────────────────────────────────────────────────────

/// Parsed markdown document
#[derive(Debug, Clone)]
pub struct MarkdownDoc {
    /// Top-level block nodes
    pub nodes: Vec<DocNode>,
}

/// Block-level document node
#[derive(Debug, Clone)]
pub enum DocNode {
    Heading {
        level: u8,
        children: Vec<InlineNode>,
    },
    Paragraph(Vec<InlineNode>),
    CodeBlock {
        lang: String,
        code: String,
    },
    Table {
        headers: Vec<TableCell>,
        rows: Vec<Vec<TableCell>>,
        aligns: Vec<Align>,
    },
    BlockQuote(Vec<DocNode>),
    OrderedList {
        start: u64,
        items: Vec<ListItem>,
    },
    UnorderedList(Vec<ListItem>),
    TaskList {
        items: Vec<TaskItem>,
    },
    ThematicBreak,
    Image {
        url: String,
        alt: String,
        title: String,
    },
    HtmlBlock(String),
    FootnoteDef {
        label: String,
        content: Vec<DocNode>,
    },
}

/// Inline node
#[derive(Debug, Clone)]
pub enum InlineNode {
    Text(String),
    Bold(Vec<InlineNode>),
    Italic(Vec<InlineNode>),
    Strikethrough(Vec<InlineNode>),
    Code(String),
    Link {
        url: String,
        title: String,
        children: Vec<InlineNode>,
    },
    Image {
        url: String,
        alt: String,
    },
    SoftBreak,
    HardBreak,
    FootnoteRef(String),
    Superscript(String),
    HtmlInline(String),
}

/// List item (ordered or unordered)
#[derive(Debug, Clone)]
pub struct ListItem {
    pub children: Vec<DocNode>,
}

/// Task list item
#[derive(Debug, Clone)]
pub struct TaskItem {
    pub checked: bool,
    pub children: Vec<DocNode>,
}

/// Table cell
#[derive(Debug, Clone)]
pub struct TableCell {
    pub content: Vec<InlineNode>,
    pub align: Align,
}

/// Table column alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

impl fmt::Display for Align {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Align::None => write!(f, "none"),
            Align::Left => write!(f, "left"),
            Align::Center => write!(f, "center"),
            Align::Right => write!(f, "right"),
        }
    }
}

// ─── Parser ─────────────────────────────────────────────────────────────────

/// Parse markdown text into MarkdownDoc
pub fn parse(text: &str) -> MarkdownDoc {
    let arena = Arena::new();
    let options = build_options();
    let root = parse_document(&arena, text, &options);
    let nodes = convert_children(root);
    MarkdownDoc { nodes }
}

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
    opts.extension.shortcodes = true;
    opts.parse.smart = true;
    opts.parse.default_info_string = Some("text".to_string());
    opts
}

fn convert_children<'a>(node: &'a AstNode<'a>) -> Vec<DocNode> {
    node.children().filter_map(convert_node).collect()
}

fn convert_node<'a>(node: &'a AstNode<'a>) -> Option<DocNode> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Document => None, // Handled by convert_children

        NodeValue::FrontMatter(_) => None, // Skip front matter display

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
            // Check if this is a task list
            let is_task = node.children().any(|child| {
                let cdata = child.data.borrow();
                if let NodeValue::TaskItem(_) = &cdata.value {
                    true
                } else {
                    false
                }
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
                            NodeValue::Item(_) => {
                                // Fallback: treat as unchecked task item
                                Some(TaskItem {
                                    checked: false,
                                    children: convert_children(child),
                                })
                            }
                            _ => None,
                        }
                    })
                    .collect();
                Some(DocNode::TaskList { items })
            } else {
                let items: Vec<ListItem> = node
                    .children()
                    .filter(|child| {
                        matches!(&child.data.borrow().value, NodeValue::Item(_))
                    })
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

        NodeValue::Image(image_node) => {
            let alt = collect_text(node);
            Some(DocNode::Image {
                url: image_node.url.clone(),
                alt,
                title: image_node.title.clone(),
            })
        }

        NodeValue::HtmlBlock(html_node) => {
            Some(DocNode::HtmlBlock(html_node.literal.clone()))
        }

        NodeValue::FootnoteDefinition(fn_def) => {
            let label = fn_def.name.clone();
            let content = convert_children(node);
            Some(DocNode::FootnoteDef { label, content })
        }

        // Skip items at block level (they are handled by List conversion)
        NodeValue::Item(_) | NodeValue::TaskItem(_) => None,

        // Skip unknown node types
        _ => None,
    }
}

fn convert_inlines<'a>(node: &'a AstNode<'a>) -> Vec<InlineNode> {
    node.children().filter_map(convert_inline).collect()
}

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
                title: link_node.title.clone(),
                children,
            })
        }

        NodeValue::Image(image_node) => {
            let alt = collect_text(node);
            Some(InlineNode::Image {
                url: image_node.url.clone(),
                alt,
            })
        }

        NodeValue::FootnoteReference(fn_ref) => {
            Some(InlineNode::FootnoteRef(fn_ref.name.clone()))
        }

        NodeValue::Superscript => {
            let text = collect_text(node);
            Some(InlineNode::Superscript(text))
        }

        NodeValue::HtmlInline(html) => Some(InlineNode::HtmlInline(html.clone())),

        // Shortcodes are rendered as emojis by comrak, appear as text
        NodeValue::Escaped => {
            let children = convert_inlines(node);
            // Escaped just means the content should be literal; merge into text
            Some(InlineNode::Text(
                children.iter().map(|n| n.plain_text()).collect(),
            ))
        }

        _ => None,
    }
}

/// Collect all plain text from an inline subtree
fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
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

impl InlineNode {
    /// Get the plain text content of this inline node
    pub fn plain_text(&self) -> String {
        match self {
            InlineNode::Text(s) => s.clone(),
            InlineNode::Bold(children)
            | InlineNode::Italic(children)
            | InlineNode::Strikethrough(children) => {
                children.iter().map(|n| n.plain_text()).collect()
            }
            InlineNode::Code(s) => s.clone(),
            InlineNode::Link { children, .. } => {
                children.iter().map(|n| n.plain_text()).collect()
            }
            InlineNode::Image { alt, .. } => alt.clone(),
            InlineNode::SoftBreak => " ".to_string(),
            InlineNode::HardBreak => "\n".to_string(),
            InlineNode::FootnoteRef(label) => format!("[^{}]", label),
            InlineNode::Superscript(s) => s.clone(),
            InlineNode::HtmlInline(s) => s.clone(),
        }
    }
}

// ─── Table conversion ───────────────────────────────────────────────────────

/// Extract table data from a comrak table node.
/// This is called from convert_node when we encounter a Table node.
/// Since comrak represents tables as special list structures, we handle it separately.

/// Convert comrak table children into our Table structure.
/// Note: comrak's table representation uses specific node types:
/// - NodeValue::Table(aligns) at the root
/// - NodeValue::TableRow(header) for rows
/// - NodeValue::TableCell for cells
pub fn convert_table<'a>(
    node: &'a AstNode<'a>,
    table: &NodeTable,
) -> Option<DocNode> {
    let mut headers = Vec::new();
    let mut rows = Vec::new();

    let aligns = &table.alignments;
    for row in node.children() {
        let row_data = row.data.borrow();
        let is_header = match &row_data.value {
            NodeValue::TableRow(header_flag) => *header_flag,
            _ => continue,
        };

        let cells: Vec<TableCell> = row
            .children()
            .enumerate()
            .map(|(i, cell)| {
                let align = aligns
                    .get(i)
                    .map(|a| match a {
                        TableAlignment::None => Align::None,
                        TableAlignment::Left => Align::Left,
                        TableAlignment::Center => Align::Center,
                        TableAlignment::Right => Align::Right,
                    })
                    .unwrap_or(Align::None);

                let content = convert_inlines(cell);
                TableCell { content, align }
            })
            .collect();

        if is_header {
            headers = cells;
        } else {
            rows.push(cells);
        }
    }

    let align_list = aligns
        .iter()
        .map(|a| match a {
            TableAlignment::None => Align::None,
            TableAlignment::Left => Align::Left,
            TableAlignment::Center => Align::Center,
            TableAlignment::Right => Align::Right,
        })
        .collect();

    Some(DocNode::Table {
        headers,
        rows,
        aligns: align_list,
    })
}

// Override convert_node to handle tables properly
fn convert_node_with_table<'a>(node: &'a AstNode<'a>) -> Option<DocNode> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Table(table) => {
            convert_table(node, table)
        }
        _ => {
            drop(data);
            convert_node(node)
        }
    }
}

// Patch convert_children to use table-aware conversion
fn convert_children_full<'a>(node: &'a AstNode<'a>) -> Vec<DocNode> {
    node.children()
        .filter_map(convert_node_with_table)
        .collect()
}

// Override the public parse to use the full converter
pub fn parse_full(text: &str) -> MarkdownDoc {
    let arena = Arena::new();
    let options = build_options();
    let root = parse_document(&arena, text, &options);
    let nodes = convert_children_full(root);
    MarkdownDoc { nodes }
}
