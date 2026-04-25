//! 内联元素渲染
//!
//! 负责将 InlineNode 转换为 egui 的 LayoutJob，处理链接点击和文本选择

use egui::*;

use crate::markdown::parser::InlineNode;
use crate::selection::TextSelector;
use crate::theme::Theme;

/// 渲染内联元素
pub fn render_inlines(
    ui: &mut Ui,
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    default_color: Color32,
    selector: &mut TextSelector,
    separator_before: &'static str,
    _id: egui::Id,
) {
    render_inlines_with_min_wrap_width(
        ui,
        inlines,
        theme,
        font_size,
        default_color,
        selector,
        separator_before,
        _id,
        100.0,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn render_inlines_with_min_wrap_width(
    ui: &mut Ui,
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    default_color: Color32,
    selector: &mut TextSelector,
    separator_before: &'static str,
    _id: egui::Id,
    min_wrap_width: f32,
) {
    let max_width = ui.available_width();
    let (job, links) = inlines_to_rich_text_with_min_wrap_width(
        inlines,
        theme,
        font_size,
        default_color,
        max_width,
        min_wrap_width,
    );
    let plain_text = job.text.clone();
    let has_links = !links.is_empty();
    let galley = ui.fonts(|f| f.layout_job(job.clone()));

    let job_for_hit_test = if has_links { Some(job.clone()) } else { None };

    let label_response = ui.add(Label::new(job).sense(Sense::click_and_drag()));
    let rect = label_response.rect;

    // 链接点击检测
    if has_links {
        handle_link_interaction(ui, &links, &label_response, &rect, job_for_hit_test);
    }

    // 处理文本选择
    selector.handle_input(ui, &label_response);
    selector.add_segment(rect, plain_text, galley, separator_before);
}

/// 处理链接交互（悬停和点击）
fn handle_link_interaction(
    ui: &mut Ui,
    links: &[(String, std::ops::Range<usize>)],
    label_response: &Response,
    rect: &Rect,
    job_for_hit_test: Option<egui::text::LayoutJob>,
) {
    let pointer_in_rect = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| rect.contains(p));
    let was_clicked = label_response.clicked();

    if pointer_in_rect || was_clicked {
        if let Some(hit_job) = job_for_hit_test {
            let galley = ui.fonts(|f| f.layout_job(hit_job));

            if pointer_in_rect {
                if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let rel = hover_pos - rect.min;
                    let char_idx = galley.cursor_from_pos(rel).ccursor.index;
                    for (_url, range) in links {
                        if range.contains(&char_idx) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                }
            }

            if was_clicked {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if rect.contains(pos) {
                        let rel = pos - rect.min;
                        let char_idx = galley.cursor_from_pos(rel).ccursor.index;
                        for (url, range) in links {
                            if range.contains(&char_idx) {
                                handle_link_click(url);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 处理链接点击
fn handle_link_click(url: &str) {
    if url.starts_with('#') {
        let anchor = url.trim_start_matches('#');
        tracing::info!("Anchor link clicked: #{}", anchor);
    } else if url.starts_with("file://")
        || url.starts_with("http://")
        || url.starts_with("https://")
    {
        let _ = open::that(url);
    } else if !url.contains("://") && !url.contains(':') {
        let _ = open::that(url);
    }
}

/// 将内联节点转换为 LayoutJob
pub fn inlines_to_rich_text(
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    default_color: Color32,
    max_width: f32,
) -> (egui::text::LayoutJob, Vec<(String, std::ops::Range<usize>)>) {
    inlines_to_rich_text_with_min_wrap_width(
        inlines,
        theme,
        font_size,
        default_color,
        max_width,
        100.0,
    )
}

pub fn inlines_to_rich_text_with_min_wrap_width(
    inlines: &[InlineNode],
    theme: &Theme,
    font_size: f32,
    default_color: Color32,
    max_width: f32,
    min_wrap_width: f32,
) -> (egui::text::LayoutJob, Vec<(String, std::ops::Range<usize>)>) {
    let mut job = egui::text::LayoutJob {
        text: String::new(),
        wrap: if max_width.is_finite() {
            egui::text::TextWrapping::wrap_at_width(max_width.max(min_wrap_width))
        } else {
            egui::text::TextWrapping::no_max_width()
        },
        ..Default::default()
    };

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

/// 字体样式状态
#[derive(Clone, Copy)]
pub struct FontStyle {
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub monospace: bool,
}

impl Default for FontStyle {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl FontStyle {
    pub const NORMAL: Self = Self {
        bold: false,
        italic: false,
        strikethrough: false,
        monospace: false,
    };
}

/// 将内联节点追加到 LayoutJob
#[allow(clippy::too_many_arguments)]
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
                let link_start = job.text.len();
                let link_style = style;
                append_inlines_to_job(
                    children, theme, font_size, theme.link, job, link_style, links, true,
                );
                let link_end = job.text.len();
                if !url.is_empty() {
                    links.push((url.clone(), link_start..link_end));
                }
            }
            InlineNode::SoftBreak => {
                push_section(job, " ", font_size, color, style, is_in_link);
            }
            InlineNode::HardBreak => {
                push_section(
                    job, "
", font_size, color, style, is_in_link,
                );
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

/// 推入文本段
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

/// 推入带背景的文本段
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
