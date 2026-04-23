//! Markdown AST → egui UI 渲染器
//!
//! 核心渲染模块，将 DocNode/InlineNode 映射为 egui 控件

mod blocks;
mod estimate;
mod inlines;

use egui::*;

use super::parser::MarkdownDoc;
use crate::image_loader::ImageLoader;
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
    image_loader: &mut ImageLoader,
    selector: &mut TextSelector,
    viewport: &mut ViewportState,
) {
    let block_count = doc.nodes.len();
    const BLOCK_SPACING: f32 = 4.0;
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
                viewport.prepare_layout(block_count, content_width, font_size);

                ui.add_space(margin);

                ui.vertical(|ui| {
                    ui.set_max_width(content_width);
                    ui.add_space(16.0);

                    let mut space_above = 0.0f32;
                    let mut current_y = 16.0f32;
                    let mut in_visible = false;

                    for (i, node) in doc.nodes.iter().enumerate() {
                        let cached_h = if let Some(block) = viewport.blocks.get_mut(i) {
                            if !block.measured {
                                block.height = estimate_block_height(node, theme, font_size);
                            }
                            block.height
                        } else {
                            estimate_block_height(node, theme, font_size)
                        };
                        let block_top = current_y;
                        let block_bottom = block_top + cached_h;

                        let is_visible = block_bottom >= vis_rect.min.y - 200.0
                            && block_top <= vis_rect.max.y + 200.0;

                        if is_visible {
                            if space_above > 0.0 {
                                ui.add_space(space_above);
                                space_above = 0.0;
                            }
                            in_visible = true;

                            let before = ui.min_rect().max.y;

                            render_block(ui, node, theme, font_size, i, image_loader, selector);

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

                            current_y += measured_h + BLOCK_SPACING;
                        } else if in_visible {
                            let remaining: f32 = viewport.blocks[i..]
                                .iter()
                                .map(|block| block.height + BLOCK_SPACING)
                                .sum();
                            ui.add_space(remaining);
                            break;
                        } else {
                            space_above += cached_h + BLOCK_SPACING;
                            current_y += cached_h + BLOCK_SPACING;
                        }
                    }

                    ui.add_space(32.0);
                });

                ui.add_space(margin);
            });
        });

    if heights_changed {
        ui.ctx().request_repaint();
    }
}
