//! 块级元素渲染
//!
//! 负责渲染标题、段落、代码块、表格、引用、列表等块级元素

use egui::*;

use super::inlines::render_inlines;
use super::table::render_table;
use crate::markdown::parser::{DocNode, InlineNode, ListItem, TaskItem};
use crate::selection::TextSelector;
use crate::theme::Theme;

/// 渲染块级节点
#[allow(clippy::too_many_arguments)]
pub fn render_block(
    ui: &mut Ui,
    node: &DocNode,
    theme: &Theme,
    font_size: f32,
    index: usize,
    selector: &mut TextSelector,
) {
    match node {
        DocNode::Heading { level, children } => {
            render_heading(ui, *level, children, theme, font_size, selector, index);
        }
        DocNode::Paragraph(inlines) => {
            render_paragraph(ui, inlines, theme, font_size, selector, index);
        }
        DocNode::CodeBlock { lang, code } => {
            render_code_block(ui, lang, code, theme, font_size, index, selector);
        }
        DocNode::Table {
            headers,
            rows,
            aligns,
        } => {
            render_table(ui, headers, rows, aligns, theme, font_size, index, selector);
        }
        DocNode::BlockQuote(children) => {
            render_block_quote(ui, children, theme, font_size, selector, index);
        }
        DocNode::OrderedList { start, items } => {
            render_ordered_list(ui, *start, items, theme, font_size, selector, index);
        }
        DocNode::UnorderedList(items) => {
            render_unordered_list(ui, items, theme, font_size, selector, index);
        }
        DocNode::TaskList { items } => {
            render_task_list(ui, items, theme, font_size, selector, index);
        }
        DocNode::ThematicBreak => {
            render_thematic_break(ui, theme);
        }
        DocNode::HtmlBlock(html) => {
            render_html_block(ui, html, theme, font_size, selector, index);
        }
        DocNode::FootnoteDef { label, content } => {
            render_footnote_def(ui, label, content, theme, font_size, selector, index);
        }
    }
}

/// 渲染标题
fn render_heading(
    ui: &mut Ui,
    level: u8,
    children: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    block_index: usize,
) {
    let size = theme.heading_size(level, font_size);
    ui.add_space(8.0);
    render_inlines(
        ui,
        children,
        theme,
        size,
        theme.heading,
        selector,
        "\n",
        ui.id().with("heading").with(block_index),
    );
    ui.add_space(4.0);
}

/// 渲染段落
fn render_paragraph(
    ui: &mut Ui,
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    block_index: usize,
) {
    render_inlines(
        ui,
        inlines,
        theme,
        font_size,
        theme.foreground,
        selector,
        "\n",
        ui.id().with("paragraph").with(block_index),
    );
    ui.add_space(8.0);
}

/// 渲染代码块
fn render_code_block(
    ui: &mut Ui,
    lang: &str,
    code: &str,
    theme: &Theme,
    font_size: f32,
    block_index: usize,
    selector: &mut TextSelector,
) {
    let code_size = font_size * 0.85;

    let frame = Frame::NONE
        .fill(theme.code_bg)
        .corner_radius(4.0)
        .inner_margin(Margin::same(12))
        .outer_margin(Margin::same(4));

    frame.show(ui, |ui| {
        if !lang.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lang)
                        .size(code_size * 0.8)
                        .color(theme.muted_text())
                        .monospace(),
                );
            });
            ui.add_space(4.0);
        }

        ScrollArea::horizontal()
            .id_salt(format!("code_scroll_{}", block_index))
            .show(ui, |ui| {
                let job = if let Some(job) = crate::markdown::highlight::highlight_code(
                    code,
                    lang,
                    &theme.syntax_theme,
                    code_size,
                ) {
                    job
                } else {
                    let mut job = egui::text::LayoutJob::default();
                    job.append(
                        code,
                        0.0,
                        TextFormat {
                            font_id: FontId::new(code_size, FontFamily::Monospace),
                            color: theme.code_fg,
                            ..Default::default()
                        },
                    );
                    job
                };

                let galley = ui.fonts(|fonts| fonts.layout_job(job.clone()));
                let response = ui.add(Label::new(job).sense(Sense::click_and_drag()));
                selector.handle_input(ui, &response);
                selector.add_segment(response.rect, code.to_string(), galley, "\n");
            });
    });
}

/// 渲染引用
#[allow(clippy::too_many_arguments)]
pub fn render_block_quote(
    ui: &mut Ui,
    children: &[DocNode],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    index: usize,
) {
    Frame::NONE
        .fill(Color32::TRANSPARENT)
        .inner_margin(0)
        .outer_margin(Margin::same(4))
        .show(ui, |ui| {
            let content_response = ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        for child in children.iter() {
                            render_block(ui, child, theme, font_size * 0.95, index, selector);
                        }
                    });
                });
            });

            let border_rect = Rect::from_min_max(
                Pos2::new(
                    content_response.response.rect.left() + 6.0,
                    content_response.response.rect.top(),
                ),
                Pos2::new(
                    content_response.response.rect.left() + 9.0,
                    content_response.response.rect.bottom(),
                ),
            );
            ui.painter()
                .rect_filled(border_rect, 2.0, theme.quote_border);
        });
}

/// 渲染有序列表
#[allow(clippy::too_many_arguments)]
pub fn render_ordered_list(
    ui: &mut Ui,
    start: u64,
    items: &[ListItem],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    block_index: usize,
) {
    for (i, item) in items.iter().enumerate() {
        let num = start + i as u64;
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("{}.", num))
                    .size(font_size)
                    .color(theme.list_marker),
            );
            ui.add_space(4.0);
            ui.vertical(|ui| {
                for child in item.children.iter() {
                    render_block(ui, child, theme, font_size, block_index, selector);
                }
            });
        });
    }
}

/// 渲染无序列表
#[allow(clippy::too_many_arguments)]
pub fn render_unordered_list(
    ui: &mut Ui,
    items: &[ListItem],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    block_index: usize,
) {
    for item in items.iter() {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(RichText::new("•").size(font_size).color(theme.list_marker));
            ui.add_space(4.0);
            ui.vertical(|ui| {
                for child in item.children.iter() {
                    render_block(ui, child, theme, font_size, block_index, selector);
                }
            });
        });
    }
}

/// 渲染任务列表
#[allow(clippy::too_many_arguments)]
pub fn render_task_list(
    ui: &mut Ui,
    items: &[TaskItem],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    index: usize,
) {
    for item in items {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(font_size, font_size), Sense::hover());
            let check_rect = rect.shrink(2.0);
            ui.painter().rect(
                check_rect,
                2.0,
                if item.checked {
                    theme.task_checked
                } else {
                    Color32::TRANSPARENT
                },
                Stroke::new(
                    1.5,
                    if item.checked {
                        theme.task_checked
                    } else {
                        theme.task_unchecked
                    },
                ),
                StrokeKind::Outside,
            );
            if item.checked {
                ui.painter().line_segment(
                    [
                        Pos2::new(check_rect.left() + 3.0, check_rect.center().y + 1.0),
                        Pos2::new(check_rect.center().x - 1.0, check_rect.bottom() - 4.0),
                    ],
                    Stroke::new(1.5, Color32::WHITE),
                );
                ui.painter().line_segment(
                    [
                        Pos2::new(check_rect.center().x - 1.0, check_rect.bottom() - 4.0),
                        Pos2::new(check_rect.right() - 3.0, check_rect.top() + 4.0),
                    ],
                    Stroke::new(1.5, Color32::WHITE),
                );
            }

            ui.add_space(8.0);
            ui.vertical(|ui| {
                for child in &item.children {
                    render_block(ui, child, theme, font_size, index, selector);
                }
            });
        });
    }
}

/// 渲染分割线
pub fn render_thematic_break(ui: &mut Ui, theme: &Theme) {
    ui.add_space(8.0);
    let rect = ui.available_rect_before_wrap();
    let line_rect = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + 8.0),
        Pos2::new(rect.right(), rect.top() + 9.0),
    );
    ui.painter().rect_filled(line_rect, 0.0, theme.hr_color);
    ui.add_space(16.0);
}

/// 渲染 HTML 块
pub fn render_html_block(
    ui: &mut Ui,
    html: &str,
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    block_index: usize,
) {
    let frame = Frame::NONE
        .fill(theme.code_bg)
        .corner_radius(4.0)
        .inner_margin(Margin::same(12));

    frame.show(ui, |ui| {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            html,
            0.0,
            TextFormat {
                font_id: FontId::new(font_size * 0.8, FontFamily::Monospace),
                color: theme.muted_text(),
                ..Default::default()
            },
        );

        let galley = ui.fonts(|fonts| fonts.layout_job(job.clone()));
        let response = ui.add(Label::new(job).sense(Sense::click_and_drag()));
        selector.handle_input(ui, &response);
        selector.add_segment(
            response.rect,
            html.to_string(),
            galley,
            if block_index == 0 { "" } else { "\n" },
        );
    });
}

/// 渲染脚注定义
#[allow(clippy::too_many_arguments)]
pub fn render_footnote_def(
    ui: &mut Ui,
    label: &str,
    content: &[DocNode],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    index: usize,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("[^{}]:", label))
                .size(font_size)
                .color(theme.link),
        );
        ui.add_space(4.0);
        ui.vertical(|ui| {
            for child in content.iter() {
                render_block(ui, child, theme, font_size, index, selector);
            }
        });
    });
}
