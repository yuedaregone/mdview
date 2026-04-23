//! 表格转换
//!
//! 将 comrak 的表格 AST 节点转换为我们的 Table 类型

use comrak::nodes::{AstNode, NodeTable, NodeValue, TableAlignment};

use super::inlines::convert_inlines;
use super::types::*;

/// 转换表格节点
pub fn convert_table<'a>(node: &'a AstNode<'a>, table: &NodeTable) -> Option<DocNode> {
    let mut headers = Vec::new();
    let mut rows = Vec::new();

    let aligns = &table.alignments;
    for row in node.children() {
        let row_data = row.data.borrow();
        let is_header = match &row_data.value {
            NodeValue::TableRow(header_flag) => header_flag,
            _ => continue,
        };

        let cells: Vec<TableCell> = row
            .children()
            .map(|cell| {
                let content = convert_inlines(cell);
                TableCell { content }
            })
            .collect();

        if *is_header {
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
