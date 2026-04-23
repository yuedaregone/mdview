//! 块级元素渲染
//!
//! 负责渲染标题、段落、代码块、表格、引用、列表等块级元素

use egui::*;

use super::inlines::render_inlines;
use crate::image_loader::{ImageLoader, ImageState};
use crate::markdown::parser::{
    Align as ParserAlign, DocNode, InlineNode, ListItem, TableCell, TaskItem,
};
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
    image_loader: &mut ImageLoader,
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
            render_code_block(ui, lang, code, theme, font_size, index);
        }
        DocNode::Table {
            headers,
            rows,
            aligns,
        } => {
            render_table(ui, headers, rows, aligns, theme, font_size, index);
        }
        DocNode::BlockQuote(children) => {
            render_block_quote(
                ui,
                children,
                theme,
                font_size,
                image_loader,
                selector,
                index,
            );
        }
        DocNode::OrderedList { start, items } => {
            render_ordered_list(
                ui,
                *start,
                items,
                theme,
                font_size,
                image_loader,
                selector,
                index,
            );
        }
        DocNode::UnorderedList(items) => {
            render_unordered_list(ui, items, theme, font_size, image_loader, selector, index);
        }
        DocNode::TaskList { items } => {
            render_task_list(ui, items, theme, font_size, image_loader, selector, index);
        }
        DocNode::ThematicBreak => {
            render_thematic_break(ui, theme);
        }
        DocNode::Image { url, alt, title } => {
            render_image(ui, url, alt, title, theme, image_loader);
        }
        DocNode::HtmlBlock(html) => {
            render_html_block(ui, html, theme, font_size);
        }
        DocNode::FootnoteDef { label, content } => {
            render_footnote_def(
                ui,
                label,
                content,
                theme,
                font_size,
                image_loader,
                selector,
                index,
            );
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
                if let Some(job) = crate::markdown::highlight::highlight_code(
                    code,
                    lang,
                    &theme.syntax_theme,
                    code_size,
                ) {
                    ui.label(job);
                } else {
                    ui.label(
                        RichText::new(code)
                            .monospace()
                            .size(code_size)
                            .color(theme.code_fg),
                    );
                }
            });
    });
}

/// 渲染表格
fn render_table(
    ui: &mut Ui,
    headers: &[TableCell],
    rows: &[Vec<TableCell>],
    alignments: &[ParserAlign],
    theme: &Theme,
    font_size: f32,
    block_index: usize,
) {
    let column_count = alignments
        .len()
        .max(headers.len())
        .max(rows.iter().map(|row| row.len()).max().unwrap_or(0));

    Frame::NONE.outer_margin(Margin::same(4)).show(ui, |ui| {
        if column_count == 0 {
            return;
        }

        let cell_rects = ScrollArea::horizontal()
            .id_salt(format!("table_scroll_{}", block_index))
            .show(ui, |ui| {
                let min_table_width = (column_count as f32 * 120.0).max(ui.available_width());
                ui.set_min_width(min_table_width);

                let mut rects = Vec::with_capacity(rows.len() + 1);

                Grid::new(ui.id().with("md_table").with(block_index))
                    .min_col_width(120.0)
                    .spacing([0.0, 0.0])
                    .show(ui, |ui| {
                        let mut header_rects = Vec::with_capacity(column_count);
                        for col_idx in 0..column_count {
                            let align = alignments
                                .get(col_idx)
                                .copied()
                                .unwrap_or(ParserAlign::None);
                            let cell_id =
                                ui.id().with("table_cell_h").with(block_index).with(col_idx);
                            let rect = render_table_cell(
                                ui,
                                headers.get(col_idx),
                                align,
                                theme,
                                font_size,
                                theme.table_header_bg,
                                cell_id,
                            );
                            header_rects.push(rect);
                        }
                        ui.end_row();
                        rects.push(header_rects);

                        for (row_idx, row) in rows.iter().enumerate() {
                            let row_bg = theme
                                .table_stripe_bg
                                .filter(|_| row_idx % 2 == 0)
                                .unwrap_or(Color32::TRANSPARENT);
                            let mut row_rects = Vec::with_capacity(column_count);

                            for col_idx in 0..column_count {
                                let align = alignments
                                    .get(col_idx)
                                    .copied()
                                    .unwrap_or(ParserAlign::None);
                                let cell_id = ui
                                    .id()
                                    .with("table_cell_d")
                                    .with(block_index)
                                    .with(row_idx)
                                    .with(col_idx);
                                let rect = render_table_cell(
                                    ui,
                                    row.get(col_idx),
                                    align,
                                    theme,
                                    font_size,
                                    row_bg,
                                    cell_id,
                                );
                                row_rects.push(rect);
                            }
                            ui.end_row();
                            rects.push(row_rects);
                        }
                    });

                rects
            })
            .inner;

        paint_table_grid(ui, &cell_rects, theme.table_border);
    });
}

/// 渲染表格单元格
fn render_table_cell(
    ui: &mut Ui,
    cell: Option<&TableCell>,
    align: ParserAlign,
    theme: &Theme,
    font_size: f32,
    background: Color32,
    cell_id: egui::Id,
) -> Rect {
    let mut selector = TextSelector::new();

    Frame::NONE
        .inner_margin(Margin::symmetric(10, 8))
        .fill(background)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));
            ui.with_layout(table_cell_layout(align), |ui| {
                if let Some(cell) = cell {
                    render_inlines(
                        ui,
                        &cell.content,
                        theme,
                        font_size,
                        theme.foreground,
                        &mut selector,
                        cell_id,
                    );
                } else {
                    ui.add_space(font_size);
                }
            });
        })
        .response
        .rect
}

fn table_cell_layout(align: ParserAlign) -> Layout {
    Layout::top_down(match align {
        ParserAlign::Center => egui::Align::Center,
        ParserAlign::Right => egui::Align::Max,
        ParserAlign::Left | ParserAlign::None => egui::Align::Min,
    })
}

fn paint_table_grid(ui: &Ui, cell_rects: &[Vec<Rect>], border: Color32) {
    let Some(first_row) = cell_rects.first() else {
        return;
    };
    let Some(first_cell) = first_row.first() else {
        return;
    };
    let Some(last_row) = cell_rects.last() else {
        return;
    };
    let Some(last_cell) = last_row.last() else {
        return;
    };

    let stroke = Stroke::new(1.0, border);
    let table_rect = Rect::from_min_max(first_cell.min, last_cell.max);
    let painter = ui.painter();

    painter.rect_stroke(table_rect, 4.0, stroke, StrokeKind::Inside);

    for row in cell_rects.iter().take(cell_rects.len().saturating_sub(1)) {
        if let Some(cell) = row.first() {
            painter.hline(table_rect.x_range(), cell.bottom(), stroke);
        }
    }

    for cell in first_row.iter().take(first_row.len().saturating_sub(1)) {
        painter.vline(cell.right(), table_rect.y_range(), stroke);
    }
}

/// 渲染引用
#[allow(clippy::too_many_arguments)]
pub fn render_block_quote(
    ui: &mut Ui,
    children: &[DocNode],
    theme: &Theme,
    font_size: f32,
    image_loader: &mut ImageLoader,
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
                            render_block(
                                ui,
                                child,
                                theme,
                                font_size * 0.95,
                                index,
                                image_loader,
                                selector,
                            );
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
    image_loader: &mut ImageLoader,
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
                    render_block(
                        ui,
                        child,
                        theme,
                        font_size,
                        block_index,
                        image_loader,
                        selector,
                    );
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
    image_loader: &mut ImageLoader,
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
                    render_block(
                        ui,
                        child,
                        theme,
                        font_size,
                        block_index,
                        image_loader,
                        selector,
                    );
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
    image_loader: &mut ImageLoader,
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
                    render_block(ui, child, theme, font_size, index, image_loader, selector);
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

/// 渲染图片
pub fn render_image(
    ui: &mut Ui,
    url: &str,
    alt: &str,
    _title: &str,
    theme: &Theme,
    image_loader: &mut ImageLoader,
) {
    let max_width = ui.available_width().min(800.0);

    match image_loader.get(url) {
        ImageState::Ready(texture_id) => {
            let size = ui
                .ctx()
                .tex_manager()
                .read()
                .meta(*texture_id)
                .unwrap()
                .size;
            let size = egui::vec2(size[0] as f32, size[1] as f32);
            let scale = if size.x > max_width {
                max_width / size.x
            } else {
                1.0
            };
            let display_size = size * scale;
            ui.add(egui::Image::new((*texture_id, display_size)));
        }
        ImageState::Loading => {
            let frame = Frame::NONE
                .fill(theme.code_bg)
                .corner_radius(4.0)
                .inner_margin(Margin::same(12));
            frame.show(ui, |ui| {
                ui.label(
                    RichText::new(format!("⏳ {}", if alt.is_empty() { url } else { alt }))
                        .size(13.0)
                        .color(theme.muted_text()),
                );
            });
        }
        ImageState::Failed(reason) => {
            let frame = Frame::NONE
                .fill(theme.code_bg)
                .corner_radius(4.0)
                .inner_margin(Margin::same(12));
            frame.show(ui, |ui| {
                let text = RichText::new(format!("❌ {}", if alt.is_empty() { url } else { alt }))
                    .size(13.0)
                    .color(theme.muted_text());
                ui.label(text).on_hover_text(reason);
            });
        }
    }
}

/// 渲染 HTML 块
pub fn render_html_block(ui: &mut Ui, html: &str, theme: &Theme, font_size: f32) {
    let frame = Frame::NONE
        .fill(theme.code_bg)
        .corner_radius(4.0)
        .inner_margin(Margin::same(12));

    frame.show(ui, |ui| {
        ui.label(
            RichText::new(html)
                .monospace()
                .size(font_size * 0.8)
                .color(theme.muted_text()),
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
    image_loader: &mut ImageLoader,
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
                render_block(ui, child, theme, font_size, index, image_loader, selector);
            }
        });
    });
}
