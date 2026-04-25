//! Markdown AST → egui UI 渲染器
//!
//! 核心渲染模块，将 DocNode/InlineNode 映射为 egui 控件

mod blocks;
mod estimate;
mod inlines;
mod table;

use egui::*;

use super::parser::MarkdownDoc;
use crate::selection::TextSelector;
use crate::theme::Theme;
use crate::viewport::ViewportState;
use blocks::render_block;
use estimate::estimate_block_height;

/// 渲染完整的 markdown 文档（带视口裁剪）
pub fn render_doc(
    ui: &mut Ui,
    doc: &MarkdownDoc,
    theme: &Theme,
    font_size: f32,
    selector: &mut TextSelector,
    viewport: &mut ViewportState,
) {
    let block_count = doc.nodes.len();
    const BLOCK_SPACING: f32 = 4.0;
    const TOP_PADDING: f32 = 16.0;
    const BOTTOM_PADDING: f32 = 32.0;
    const OVERSCAN: f32 = 200.0;
    let mut heights_changed = false;

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .drag_to_scroll(false)
        .show_viewport(ui, |ui, vis_rect| {
            ui.horizontal_top(|ui| {
                let total_width = ui.available_width();
                let max_width = 800.0;
                let content_width = total_width.min(max_width);
                let margin = (total_width - content_width) / 2.0;
                let layout_reset = viewport.prepare_layout(block_count, content_width, font_size);
                if layout_reset {
                    for (block, node) in viewport.blocks.iter_mut().zip(doc.nodes.iter()) {
                        block.height = estimate_block_height(node, theme, font_size);
                    }
                }
                viewport.rebuild_positions(TOP_PADDING, BLOCK_SPACING, BOTTOM_PADDING);

                ui.add_space(margin);

                ui.vertical(|ui| {
                    ui.set_max_width(content_width);
                    let visible_range =
                        viewport.visible_range(vis_rect.min.y, vis_rect.max.y, OVERSCAN);

                    if visible_range.is_empty() {
                        ui.add_space(viewport.total_height());
                        return;
                    }

                    let leading_space = viewport.offset_before(visible_range.start);
                    if leading_space > 0.0 {
                        ui.add_space(leading_space);
                    }

                    for i in visible_range.clone() {
                        let node = &doc.nodes[i];

                        let before = ui.min_rect().max.y;

                        render_block(ui, node, theme, font_size, i, selector);

                        ui.add_space(BLOCK_SPACING);
                        let actual_h = (ui.min_rect().max.y - before - BLOCK_SPACING).max(0.0);
                        let measured_h = actual_h.max(1.0);

                        if let Some(block) = viewport.blocks.get_mut(i) {
                            if !block.measured || (block.height - measured_h).abs() > 0.5 {
                                block.height = measured_h;
                                heights_changed = true;
                            }
                            if !block.measured {
                                block.measured = true;
                            }
                        }
                    }

                    let trailing_space = viewport.trailing_space_from(visible_range.end);
                    if trailing_space > 0.0 {
                        ui.add_space(trailing_space);
                    }
                });

                ui.add_space(margin);
            });
        });

    if heights_changed {
        viewport.mark_layout_dirty();
        // 只有当高度变化超过一定阈值，或者已经显示过第一帧后才请求重绘
        // 这能减少启动时的 Layout Shift 导致的视觉闪烁
        ui.ctx().request_repaint();
    }
}
