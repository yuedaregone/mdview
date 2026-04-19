//! Markdown AST → egui UI renderer
//!
//! Core rendering module that maps DocNode/InlineNode to egui widgets.

use egui::*;

use super::parser::{Align, DocNode, InlineNode, ListItem, MarkdownDoc, TableCell, TaskItem};
use crate::image_loader::{ImageLoader, ImageState};
use crate::selection::TextSelector;
use crate::theme::Theme;
use crate::viewport::ViewportState;

/// Render a complete markdown document with viewport culling
pub fn render_doc(
    ui: &mut Ui,
    doc: &MarkdownDoc,
    theme: &Theme,
    font_size: f32,
    image_loader: &mut ImageLoader,
    selector: &mut TextSelector,
    viewport: &mut ViewportState,
) {
    let block_count = doc.nodes.len();

    // Update viewport state with latest dimensions
    viewport.scroll_offset = 0.0; // Will be set by app.rs
    viewport.viewport_height = ui.max_rect().height();

    // Reinitialize if block count changed
    if viewport.blocks.len() != block_count {
        *viewport = ViewportState::new(block_count);
    }

    // Render all blocks - viewport culling disabled for stability
    // The ScrollArea handles scrolling, we just render all content
    for (i, node) in doc.nodes.iter().enumerate() {
        let before = ui.cursor().min.y;
        render_block(ui, node, theme, font_size, i, image_loader, selector);
        let after = ui.cursor().min.y;

        // Only update height if not yet measured
        if let Some(block) = viewport.blocks.get_mut(i) {
            if !block.measured {
                let height = (after - before).max(20.0);
                block.height = height;
                block.measured = true;
            }
        }
        ui.add_space(4.0);
    }
    ui.add_space(32.0);
}

/// Render a block-level node
fn render_block(
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
            render_block_quote(ui, children, theme, font_size, image_loader, selector);
        }
        DocNode::OrderedList { start, items } => {
            render_ordered_list(ui, *start, items, theme, font_size, image_loader, selector);
        }
        DocNode::UnorderedList(items) => {
            render_unordered_list_ui(ui, items, theme, font_size, image_loader, selector);
        }
        DocNode::TaskList { items } => {
            render_task_list(ui, items, theme, font_size, image_loader, selector);
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
            render_footnote_def(ui, label, content, theme, font_size, image_loader, selector);
        }
    }
}

// ─── Heading ────────────────────────────────────────────────────────────────

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
    let before = ui.cursor().min;
    render_inlines(
        ui,
        children,
        theme,
        size,
        theme.heading,
        selector,
        block_index,
    );
    let after = ui.cursor().min;
    // Record text segment for selection
    let plain = children.iter().map(|n| n.plain_text()).collect::<String>();
    let rect = egui::Rect::from_min_max(before, egui::pos2(ui.max_rect().right(), after.y));
    selector.add_segment(rect, plain, 0);
    ui.add_space(4.0);
}

// ─── Paragraph ──────────────────────────────────────────────────────────────

fn render_paragraph(
    ui: &mut Ui,
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    block_index: usize,
) {
    let before = ui.cursor().min;
    render_inlines(
        ui,
        inlines,
        theme,
        font_size,
        theme.foreground,
        selector,
        block_index,
    );
    let after = ui.cursor().min;
    let plain = inlines.iter().map(|n| n.plain_text()).collect::<String>();
    let rect = egui::Rect::from_min_max(before, egui::pos2(ui.max_rect().right(), after.y));
    selector.add_segment(rect, plain, 0);
    ui.add_space(8.0);
}

// ─── Code Block ─────────────────────────────────────────────────────────────

fn render_code_block(
    ui: &mut Ui,
    lang: &str,
    code: &str,
    theme: &Theme,
    font_size: f32,
    block_index: usize,
) {
    let code_size = font_size * 0.85;

    // Background frame
    let frame = Frame::NONE
        .fill(theme.code_bg)
        .corner_radius(4.0)
        .inner_margin(Margin::same(12))
        .outer_margin(Margin::same(4));

    frame.show(ui, |ui| {
        // Language label
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

        // Code content with horizontal scrolling
        ScrollArea::horizontal()
            .id_salt(format!("code_scroll_{}", block_index))
            .show(ui, |ui| {
                // Try syntax highlighting → LayoutJob
                if let Some(job) = crate::markdown::highlight::highlight_code(
                    code,
                    lang,
                    theme.syntax_theme,
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

// ─── Table ──────────────────────────────────────────────────────────────────

fn render_table(
    ui: &mut Ui,
    headers: &[TableCell],
    rows: &[Vec<TableCell>],
    aligns: &[Align],
    theme: &Theme,
    font_size: f32,
    block_index: usize,
) {
    let frame = Frame::NONE
        .stroke(Stroke::new(1.0, theme.table_border))
        .corner_radius(4.0)
        .inner_margin(0)
        .outer_margin(Margin::same(4));

    frame.show(ui, |ui| {
        ScrollArea::horizontal()
            .id_salt(format!("table_scroll_{}", block_index))
            .show(ui, |ui| {
                let _num_cols = aligns.len().max(headers.len());

                Grid::new(format!("md_table_{}", block_index))
                    .striped(theme.table_stripe_bg.is_some())
                    .min_col_width(60.0)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        // Header row
                        for (i, cell) in headers.iter().enumerate() {
                            let align = aligns.get(i).copied().unwrap_or(Align::None);
                            render_table_cell(ui, cell, align, theme, font_size, true);
                        }
                        ui.end_row();

                        // Data rows
                        for row in rows.iter() {
                            for (i, cell) in row.iter().enumerate() {
                                let align = aligns.get(i).copied().unwrap_or(Align::None);
                                render_table_cell(ui, cell, align, theme, font_size, false);
                            }
                            ui.end_row();
                        }
                    });
            });
    });
}

static TABLE_CELL_INDEX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn render_table_cell(
    ui: &mut Ui,
    cell: &TableCell,
    align: Align,
    theme: &Theme,
    font_size: f32,
    is_header: bool,
) {
    let cell_index = TABLE_CELL_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let layout = match align {
        Align::Center => Layout::centered_and_justified(egui::Direction::TopDown),
        Align::Right => Layout::right_to_left(egui::Align::Center),
        _ => Layout::left_to_right(egui::Align::Center),
    };

    ui.with_layout(layout, |ui| {
        Frame::NONE
            .inner_margin(Margin::same(8))
            .fill(if is_header {
                theme.table_header_bg
            } else {
                Color32::TRANSPARENT
            })
            .show(ui, |ui| {
                render_inlines(
                    ui,
                    &cell.content,
                    theme,
                    font_size,
                    theme.foreground,
                    &mut TextSelector::new(),
                    cell_index,
                );
            });
    });
}

// ─── Block Quote ────────────────────────────────────────────────────────────

fn render_block_quote(
    ui: &mut Ui,
    children: &[DocNode],
    theme: &Theme,
    font_size: f32,
    image_loader: &mut ImageLoader,
    selector: &mut TextSelector,
) {
    Frame::NONE
        .fill(Color32::TRANSPARENT)
        .inner_margin(0)
        .outer_margin(Margin::same(4))
        .show(ui, |ui| {
            // Render content first, then overlay the left border
            let content_response = ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(16.0); // Space for left border + padding
                    ui.vertical(|ui| {
                        for child in children {
                            render_block(
                                ui,
                                child,
                                theme,
                                font_size * 0.95,
                                0,
                                image_loader,
                                selector,
                            );
                        }
                    });
                });
            });

            // Draw left border over the content area
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

// ─── Lists ──────────────────────────────────────────────────────────────────

static LIST_ITEM_INDEX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn render_ordered_list(
    ui: &mut Ui,
    start: u64,
    items: &[ListItem],
    theme: &Theme,
    font_size: f32,
    image_loader: &mut ImageLoader,
    selector: &mut TextSelector,
) {
    for (i, item) in items.iter().enumerate() {
        let mut idx = LIST_ITEM_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
                for child in &item.children {
                    render_block(ui, child, theme, font_size, idx, image_loader, selector);
                    idx += 1;
                }
            });
        });
    }
}

fn render_unordered_list_ui(
    ui: &mut Ui,
    items: &[ListItem],
    theme: &Theme,
    font_size: f32,
    image_loader: &mut ImageLoader,
    selector: &mut TextSelector,
) {
    for item in items {
        let mut idx = LIST_ITEM_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(RichText::new("•").size(font_size).color(theme.list_marker));
            ui.add_space(4.0);
            ui.vertical(|ui| {
                for child in &item.children {
                    render_block(ui, child, theme, font_size, idx, image_loader, selector);
                    idx += 1;
                }
            });
        });
    }
}

fn render_task_list(
    ui: &mut Ui,
    items: &[TaskItem],
    theme: &Theme,
    font_size: f32,
    image_loader: &mut ImageLoader,
    selector: &mut TextSelector,
) {
    for item in items {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            // Checkbox
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
                // Draw checkmark
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
                    render_block(ui, child, theme, font_size, 0, image_loader, selector);
                }
            });
        });
    }
}

// ─── Thematic Break ─────────────────────────────────────────────────────────

fn render_thematic_break(ui: &mut Ui, theme: &Theme) {
    ui.add_space(8.0);
    let rect = ui.available_rect_before_wrap();
    let line_rect = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + 8.0),
        Pos2::new(rect.right(), rect.top() + 9.0),
    );
    ui.painter().rect_filled(line_rect, 0.0, theme.hr_color);
    ui.add_space(16.0);
}

// ─── Image ──────────────────────────────────────────────────────────────────

fn render_image(
    ui: &mut Ui,
    url: &str,
    alt: &str,
    _title: &str,
    theme: &Theme,
    image_loader: &mut ImageLoader,
) {
    let max_width = ui.available_width().min(800.0);

    match image_loader.get(url) {
        ImageState::Ready(texture) => {
            let size = texture.size_vec2();
            let scale = if size.x > max_width {
                max_width / size.x
            } else {
                1.0
            };
            let display_size = size * scale;
            ui.add(
                egui::Image::from_texture(&*texture)
                    .max_width(display_size.x)
                    .fit_to_original_size(scale),
            );
        }
        ImageState::Loading => {
            // Show loading placeholder
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
        ImageState::Failed => {
            // Show error placeholder
            let frame = Frame::NONE
                .fill(theme.code_bg)
                .corner_radius(4.0)
                .inner_margin(Margin::same(12));
            frame.show(ui, |ui| {
                ui.label(
                    RichText::new(format!("❌ {}", if alt.is_empty() { url } else { alt }))
                        .size(13.0)
                        .color(theme.muted_text()),
                );
            });
        }
    }
}

// ─── HTML Block ─────────────────────────────────────────────────────────────

fn render_html_block(ui: &mut Ui, html: &str, theme: &Theme, font_size: f32) {
    // Just display raw HTML as monospace text
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

// ─── Footnote Definition ────────────────────────────────────────────────────

fn render_footnote_def(
    ui: &mut Ui,
    label: &str,
    content: &[DocNode],
    theme: &Theme,
    font_size: f32,
    image_loader: &mut ImageLoader,
    selector: &mut TextSelector,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("[^{}]:", label))
                .size(font_size)
                .color(theme.link),
        );
        ui.add_space(4.0);
        ui.vertical(|ui| {
            for child in content {
                render_block(ui, child, theme, font_size, 0, image_loader, selector);
            }
        });
    });
}

// ─── Inline Rendering ───────────────────────────────────────────────────────

/// Render inline nodes as a single text block with link clicking support.
/// Uses ui.label() for native selection, ui.interact() for link hit testing.
fn render_inlines(
    ui: &mut Ui,
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    default_color: Color32,
    selector: &mut TextSelector,
    block_index: usize,
) {
    let max_width = ui.available_width();
    let (job, links) = inlines_to_rich_text(inlines, theme, font_size, default_color, max_width);
    let plain_text = job.text.clone();

    // Layout for link hit testing
    let galley = if !links.is_empty() {
        Some(ui.fonts(|f| f.layout_job(job.clone())))
    } else {
        None
    };

    // Use Label: preserve native text selection
    let label_response = ui.label(job);
    let rect = label_response.rect;

    // Link hit test using raw input
    if let Some(ref galley) = galley {
        // Hover detection
        if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
            if rect.contains(hover_pos) {
                let rel = hover_pos - rect.min;
                let char_idx = galley.cursor_from_pos(rel).ccursor.index;

                for (url, range) in &links {
                    if range.contains(&char_idx) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            }
        }

        // Click detection using different approach: check on DOWN event
        let down = ui.input(|i| i.pointer.primary_down());
        let interact_pos = ui.input(|i| i.pointer.interact_pos());

        // When mouse goes DOWN on a link, we'll open it
        if down {
            if let Some(pos) = interact_pos {
                if rect.contains(pos) {
                    eprintln!("DEBUG: mouse down on link rect={:?}, pos={:?}", rect, pos);
                    let rel = pos - rect.min;
                    let char_idx = galley.cursor_from_pos(rel).ccursor.index;

                    for (url, range) in &links {
                        if range.contains(&char_idx) {
                            eprintln!("DEBUG: mouse down on URL: {}", url);
                            // Open immediately on mouse down
                            let _ = open::that(url);
                        }
                    }
                }
            }
        }
    }

    selector.add_segment(rect, plain_text, 0);
}

/// Convert inline nodes to a LayoutJob with proper inline formatting.
/// Supports bold, italic, strikethrough, inline code, links within a line.
/// Returns the LayoutJob and a list of (url, char_range) for clickable links.
/// Note: char_range uses character indices (matching cursor.ccursor.index), not byte offsets.
fn inlines_to_rich_text(
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    default_color: Color32,
    max_width: f32,
) -> (egui::text::LayoutJob, Vec<(String, std::ops::Range<usize>)>) {
    let mut job = egui::text::LayoutJob::default();
    job.text = String::new();
    job.wrap = egui::text::TextWrapping::wrap_at_width(max_width.max(100.0));

    let mut links = Vec::new();
    append_inlines_to_job(
        inlines,
        theme,
        font_size,
        default_color,
        &mut job,
        FontStyle::NORMAL,
        &mut links,
        false,
    );

    (job, links)
}

/// Font style state for inline rendering
#[derive(Clone, Copy)]
struct FontStyle {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    monospace: bool,
}

impl Default for FontStyle {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl FontStyle {
    const NORMAL: Self = Self {
        bold: false,
        italic: false,
        strikethrough: false,
        monospace: false,
    };
}

/// Append inline nodes to a LayoutJob with formatting state
fn append_inlines_to_job(
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    color: Color32,
    job: &mut egui::text::LayoutJob,
    style: FontStyle,
    links: &mut Vec<(String, std::ops::Range<usize>)>,
    is_in_link: bool,
) {
    for inline in inlines {
        match inline {
            InlineNode::Text(s) => {
                push_section(job, s, font_size, color, style, is_in_link);
            }
            InlineNode::Bold(children) => {
                let mut s = style;
                s.bold = true;
                append_inlines_to_job(children, theme, font_size, color, job, s, links, is_in_link);
            }
            InlineNode::Italic(children) => {
                let mut s = style;
                s.italic = true;
                append_inlines_to_job(children, theme, font_size, color, job, s, links, is_in_link);
            }
            InlineNode::Strikethrough(children) => {
                let mut s = style;
                s.strikethrough = true;
                append_inlines_to_job(children, theme, font_size, color, job, s, links, is_in_link);
            }
            InlineNode::Code(s) => {
                let mut code_style = style;
                code_style.monospace = true;
                push_section_with_bg(
                    job,
                    s,
                    font_size * 0.9,
                    theme.code_fg,
                    code_style,
                    false,
                    theme.code_bg,
                );
            }
            InlineNode::Link { url, children, .. } => {
                let link_start = job.text.chars().count();
                let link_style = style;
                append_inlines_to_job(
                    children, theme, font_size, theme.link, job, link_style, links, true,
                );
                let link_end = job.text.chars().count();
                if !url.is_empty() {
                    links.push((url.clone(), link_start..link_end));
                }
            }
            InlineNode::Image { alt, .. } => {
                push_section(
                    job,
                    &format!("🖼 {}", alt),
                    font_size * 0.9,
                    theme.muted_text(),
                    FontStyle::NORMAL,
                    false,
                );
            }
            InlineNode::SoftBreak => {
                push_section(job, " ", font_size, color, style, is_in_link);
            }
            InlineNode::HardBreak => {
                push_section(job, "\n", font_size, color, style, is_in_link);
            }
            InlineNode::FootnoteRef(label) => {
                push_section(
                    job,
                    &format!("[^{}]", label),
                    font_size * 0.85,
                    theme.link,
                    FontStyle::NORMAL,
                    false,
                );
            }
            InlineNode::Superscript(s) => {
                push_section(job, s, font_size * 0.7, color, FontStyle::NORMAL, false);
            }
            InlineNode::HtmlInline(s) => {
                push_section(
                    job,
                    s,
                    font_size,
                    theme.muted_text(),
                    FontStyle::NORMAL,
                    false,
                );
            }
        }
    }
}

/// Push a text section into the LayoutJob
fn push_section(
    job: &mut egui::text::LayoutJob,
    text: &str,
    font_size: f32,
    color: Color32,
    style: FontStyle,
    is_link: bool,
) {
    push_section_with_bg(
        job,
        text,
        font_size,
        color,
        style,
        is_link,
        Color32::TRANSPARENT,
    );
}

/// Push a text section into the LayoutJob with optional background color
fn push_section_with_bg(
    job: &mut egui::text::LayoutJob,
    text: &str,
    font_size: f32,
    color: Color32,
    style: FontStyle,
    is_link: bool,
    bg: Color32,
) {
    if text.is_empty() {
        return;
    }

    let start = job.text.len();
    job.text.push_str(text);
    let end = job.text.len();

    // For bold, use a larger font size as egui doesn't have a bold flag
    let effective_size = if style.bold {
        font_size * 1.05
    } else {
        font_size
    };

    let font_family = if style.monospace {
        egui::FontFamily::Monospace
    } else {
        egui::FontFamily::Proportional
    };

    job.sections.push(egui::text::LayoutSection {
        leading_space: 0.0,
        byte_range: start..end,
        format: egui::TextFormat {
            font_id: egui::FontId::new(effective_size, font_family),
            color,
            background: bg,
            italics: style.italic,
            strikethrough: if style.strikethrough {
                egui::Stroke::new(1.0, color)
            } else {
                egui::Stroke::NONE
            },
            underline: if is_link {
                egui::Stroke::new(1.0, color)
            } else {
                egui::Stroke::NONE
            },
            ..Default::default()
        },
    });
}
