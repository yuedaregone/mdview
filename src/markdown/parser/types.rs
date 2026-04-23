//! Markdown 解析器：公共类型定义

use std::fmt;

/// 解析后的 markdown 文档
#[derive(Debug, Clone)]
pub struct MarkdownDoc {
    /// 顶层块级节点
    pub nodes: Vec<DocNode>,
}

/// 块级文档节点
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
    HtmlBlock(String),
    FootnoteDef {
        label: String,
        content: Vec<DocNode>,
    },
}

/// 内联节点
#[derive(Debug, Clone)]
pub enum InlineNode {
    Text(String),
    Bold(Vec<InlineNode>),
    Italic(Vec<InlineNode>),
    Strikethrough(Vec<InlineNode>),
    Code(String),
    Link {
        url: String,
        children: Vec<InlineNode>,
    },
    SoftBreak,
    HardBreak,
    FootnoteRef(String),
    Superscript(String),
    HtmlInline(String),
}

/// 列表项（有序或无序）
#[derive(Debug, Clone)]
pub struct ListItem {
    pub children: Vec<DocNode>,
}

/// 任务列表项
#[derive(Debug, Clone)]
pub struct TaskItem {
    pub checked: bool,
    pub children: Vec<DocNode>,
}

/// 表格单元格
#[derive(Debug, Clone)]
pub struct TableCell {
    pub content: Vec<InlineNode>,
}

/// 表格列对齐方式
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

impl InlineNode {
    /// 获取内联节点的纯文本内容
    pub fn plain_text(&self) -> String {
        match self {
            InlineNode::Text(s) => s.clone(),
            InlineNode::Bold(children)
            | InlineNode::Italic(children)
            | InlineNode::Strikethrough(children) => {
                children.iter().map(|n| n.plain_text()).collect()
            }
            InlineNode::Code(s) => s.clone(),
            InlineNode::Link { children, .. } => children.iter().map(|n| n.plain_text()).collect(),
            InlineNode::SoftBreak => " ".to_string(),
            InlineNode::HardBreak => "\n".to_string(),
            InlineNode::FootnoteRef(label) => format!("[^{}]", label),
            InlineNode::Superscript(s) => s.clone(),
            InlineNode::HtmlInline(s) => s.clone(),
        }
    }
}
