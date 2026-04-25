//! Table renderer
//!
//! Isolates table layout, measurement, painting, and selection wiring.

use egui::*;

use super::inlines::{
    inlines_to_rich_text, inlines_to_rich_text_with_min_wrap_width,
    render_inlines_with_min_wrap_width,
};
use crate::markdown::parser::{Align as ParserAlign, TableCell};
use crate::selection::TextSelector;
use crate::theme::Theme;

const CELL_PADDING_X: f32 = 10.0;
const CELL_PADDING_Y: f32 = 8.0;
const MIN_COLUMN_WIDTH: f32 = 88.0;
const BASE_MAX_COLUMN_WIDTH: f32 = 420.0;
const MIN_CELL_WRAP_WIDTH: f32 = 32.0;

pub(super) fn render_table(
    ui: &mut Ui,
    headers: &[TableCell],
    rows: &[Vec<TableCell>],
    alignments: &[ParserAlign],
    theme: &Theme,
    font_size: f32,
    block_index: usize,
    selector: &mut TextSelector,
) {
    let available_width = ui.available_width();
    let column_count = alignments
        .len()
        .max(headers.len())
        .max(rows.iter().map(|row| row.len()).max().unwrap_or(0));
    let column_widths = estimate_table_column_widths(
        ui,
        headers,
        rows,
        column_count,
        theme,
        font_size,
        available_width,
    );
    let row_heights = measure_table_row_heights(
        ui,
        headers,
        rows,
        &column_widths,
        theme,
        font_size,
        CELL_PADDING_X,
        CELL_PADDING_Y,
    );

    Frame::NONE.outer_margin(Margin::same(4)).show(ui, |ui| {
        if column_count == 0 {
            return;
        }

        ScrollArea::horizontal()
            .id_salt(format!("table_scroll_{}", block_index))
            .show(ui, |ui| {
                let min_table_width = column_widths.iter().sum::<f32>().max(ui.available_width());
                ui.set_min_width(min_table_width);

                let mut rects = Vec::with_capacity(rows.len() + 1);

                Grid::new(ui.id().with("md_table").with(block_index))
                    .min_col_width(0.0)
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
                                column_widths[col_idx],
                                row_heights[0],
                                CELL_PADDING_X,
                                CELL_PADDING_Y,
                                theme.table_header_bg,
                                selector,
                                if col_idx == 0 { "\n" } else { "\t" },
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
                                    column_widths[col_idx],
                                    row_heights[row_idx + 1],
                                    CELL_PADDING_X,
                                    CELL_PADDING_Y,
                                    row_bg,
                                    selector,
                                    if col_idx == 0 { "\n" } else { "\t" },
                                    cell_id,
                                );
                                row_rects.push(rect);
                            }
                            ui.end_row();
                            rects.push(row_rects);
                        }
                    });

                paint_table_grid(ui, &rects, theme.table_border);
                rects
            });
    });
}

fn render_table_cell(
    ui: &mut Ui,
    cell: Option<&TableCell>,
    align: ParserAlign,
    theme: &Theme,
    font_size: f32,
    min_width: f32,
    target_height: f32,
    padding_x: f32,
    padding_y: f32,
    background: Color32,
    selector: &mut TextSelector,
    separator_before: &'static str,
    cell_id: Id,
) -> Rect {
    let (cell_rect, _) = ui.allocate_exact_size(
        vec2(min_width.max(1.0), target_height.max(font_size)),
        Sense::hover(),
    );

    if background != Color32::TRANSPARENT {
        ui.painter().rect_filled(cell_rect, 0.0, background);
    }

    let inner_rect = Rect::from_min_max(
        Pos2::new(cell_rect.left() + padding_x, cell_rect.top() + padding_y),
        Pos2::new(
            cell_rect.right() - padding_x,
            cell_rect.bottom() - padding_y,
        ),
    );

    let mut child_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(inner_rect)
            .layout(table_cell_layout(align)),
    );

    if let Some(cell) = cell {
        render_inlines_with_min_wrap_width(
            &mut child_ui,
            &cell.content,
            theme,
            font_size,
            theme.foreground,
            selector,
            separator_before,
            cell_id,
            MIN_CELL_WRAP_WIDTH,
        );
    }

    cell_rect
}

fn table_cell_layout(align: ParserAlign) -> Layout {
    Layout::top_down(match align {
        ParserAlign::Center => Align::Center,
        ParserAlign::Right => Align::Max,
        ParserAlign::Left | ParserAlign::None => Align::Min,
    })
}

fn paint_table_grid(ui: &Ui, cell_rects: &[Vec<Rect>], border: Color32) {
    let Some(first_row) = cell_rects.first() else {
        return;
    };
    if first_row.is_empty() {
        return;
    }
    let Some(last_row) = cell_rects.last() else {
        return;
    };
    let Some(last_cell) = last_row.last() else {
        return;
    };

    let stroke = Stroke::new(1.0, border);
    let row_bounds: Vec<(f32, f32)> = cell_rects
        .iter()
        .map(|row| {
            row.iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(top, bottom), rect| {
                    (top.min(rect.top()), bottom.max(rect.bottom()))
                })
        })
        .collect();
    let col_bounds: Vec<(f32, f32)> = (0..first_row.len())
        .map(|col| {
            cell_rects
                .iter()
                .filter_map(|row| row.get(col))
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(left, right), rect| {
                    (left.min(rect.left()), right.max(rect.right()))
                })
        })
        .collect();

    let table_rect = Rect::from_min_max(
        Pos2::new(col_bounds[0].0, row_bounds[0].0),
        Pos2::new(
            col_bounds
                .last()
                .map(|(_, right)| *right)
                .unwrap_or(last_cell.right()),
            row_bounds
                .last()
                .map(|(_, bottom)| *bottom)
                .unwrap_or(last_cell.bottom()),
        ),
    );
    let painter = ui.painter();

    painter.rect_stroke(table_rect, 4.0, stroke, StrokeKind::Inside);

    for (_, bottom) in row_bounds.iter().take(row_bounds.len().saturating_sub(1)) {
        painter.hline(table_rect.x_range(), *bottom, stroke);
    }

    for (_, right) in col_bounds.iter().take(col_bounds.len().saturating_sub(1)) {
        painter.vline(*right, table_rect.y_range(), stroke);
    }
}

fn measure_table_row_heights(
    ui: &Ui,
    headers: &[TableCell],
    rows: &[Vec<TableCell>],
    column_widths: &[f32],
    theme: &Theme,
    font_size: f32,
    padding_x: f32,
    padding_y: f32,
) -> Vec<f32> {
    let mut heights = Vec::with_capacity(rows.len() + 1);
    let content_widths: Vec<f32> = column_widths
        .iter()
        .map(|width| (width - padding_x * 2.0).max(1.0))
        .collect();

    let header_height = (0..column_widths.len())
        .map(|col_idx| {
            measure_table_cell_height(
                ui,
                headers.get(col_idx),
                content_widths[col_idx],
                theme,
                font_size,
                padding_y,
            )
        })
        .fold(0.0, f32::max);
    heights.push(header_height);

    for row in rows {
        let row_height = (0..column_widths.len())
            .map(|col_idx| {
                measure_table_cell_height(
                    ui,
                    row.get(col_idx),
                    content_widths[col_idx],
                    theme,
                    font_size,
                    padding_y,
                )
            })
            .fold(0.0, f32::max);
        heights.push(row_height);
    }

    heights
}

fn measure_table_cell_height(
    ui: &Ui,
    cell: Option<&TableCell>,
    content_width: f32,
    theme: &Theme,
    font_size: f32,
    padding_y: f32,
) -> f32 {
    let Some(cell) = cell else {
        return font_size + padding_y * 2.0;
    };

    let (job, _) = inlines_to_rich_text_with_min_wrap_width(
        &cell.content,
        theme,
        font_size,
        theme.foreground,
        content_width,
        MIN_CELL_WRAP_WIDTH,
    );
    let galley = ui.fonts(|fonts| fonts.layout_job(job));

    galley.size().y.max(font_size) + padding_y * 2.0
}

fn estimate_table_column_widths(
    ui: &Ui,
    headers: &[TableCell],
    rows: &[Vec<TableCell>],
    column_count: usize,
    theme: &Theme,
    font_size: f32,
    available_width: f32,
) -> Vec<f32> {
    let mut widths = vec![MIN_COLUMN_WIDTH; column_count];
    let soft_max_width = BASE_MAX_COLUMN_WIDTH.max(available_width * 0.55);

    for col_idx in 0..column_count {
        let header_width = headers
            .get(col_idx)
            .map(|cell| estimate_cell_width(ui, cell, theme, font_size, true))
            .unwrap_or(0.0);
        let row_width = rows
            .iter()
            .filter_map(|row| row.get(col_idx))
            .map(|cell| estimate_cell_width(ui, cell, theme, font_size, false))
            .fold(0.0, f32::max);

        widths[col_idx] = header_width
            .max(row_width)
            .clamp(MIN_COLUMN_WIDTH, soft_max_width);
    }

    fit_column_widths(widths, available_width)
}

fn estimate_cell_width(
    ui: &Ui,
    cell: &TableCell,
    theme: &Theme,
    font_size: f32,
    is_header: bool,
) -> f32 {
    let measured_width = measure_table_cell_width(ui, cell, theme, font_size);
    let visual_bias = if is_header { 1.04 } else { 1.0 };

    (measured_width * visual_bias + CELL_PADDING_X * 2.0).max(MIN_COLUMN_WIDTH)
}

fn measure_table_cell_width(ui: &Ui, cell: &TableCell, theme: &Theme, font_size: f32) -> f32 {
    let (job, _) = inlines_to_rich_text(
        &cell.content,
        theme,
        font_size,
        theme.foreground,
        f32::INFINITY,
    );
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    galley.size().x.max(1.0)
}

fn adaptive_min_column_width(column_count: usize, target_total_width: f32) -> f32 {
    if column_count == 0 || !target_total_width.is_finite() || target_total_width <= 0.0 {
        return MIN_COLUMN_WIDTH;
    }

    let even_width = target_total_width / column_count as f32;
    even_width.clamp(MIN_CELL_WRAP_WIDTH + CELL_PADDING_X * 2.0, MIN_COLUMN_WIDTH)
}

fn fit_column_widths(widths: Vec<f32>, target_total_width: f32) -> Vec<f32> {
    if widths.is_empty() {
        return widths;
    }

    let current_total = widths.iter().sum::<f32>();
    if current_total >= target_total_width {
        shrink_column_widths(widths, target_total_width)
    } else {
        expand_column_widths(widths, target_total_width)
    }
}

fn expand_column_widths(mut widths: Vec<f32>, target_total_width: f32) -> Vec<f32> {
    if widths.is_empty() || !target_total_width.is_finite() || target_total_width <= 0.0 {
        return widths;
    }

    let current_total = widths.iter().sum::<f32>();
    if current_total >= target_total_width {
        return widths;
    }

    let extra = target_total_width - current_total;
    if current_total <= f32::EPSILON {
        let even_extra = extra / widths.len() as f32;
        for width in &mut widths {
            *width += even_extra;
        }
    } else {
        for width in &mut widths {
            *width += extra * (*width / current_total);
        }
    }

    let adjusted_total = widths.iter().sum::<f32>();
    if let Some(last) = widths.last_mut() {
        *last += target_total_width - adjusted_total;
    }

    widths
}

fn shrink_column_widths(mut widths: Vec<f32>, target_total_width: f32) -> Vec<f32> {
    if widths.is_empty() || !target_total_width.is_finite() || target_total_width <= 0.0 {
        return widths;
    }

    let current_total = widths.iter().sum::<f32>();
    if current_total <= target_total_width {
        return widths;
    }

    let min_width = adaptive_min_column_width(widths.len(), target_total_width);
    let min_widths: Vec<f32> = widths.iter().map(|width| width.min(min_width)).collect();
    let min_total = min_widths.iter().sum::<f32>();

    if min_total >= target_total_width {
        let scale = target_total_width / min_total;
        for (width, min_width) in widths.iter_mut().zip(min_widths.iter()) {
            *width = (*min_width * scale).max(1.0);
        }
        normalize_column_widths(&mut widths, target_total_width);
        return widths;
    }

    let mut remaining_deficit = current_total - target_total_width;
    let mut flexible: Vec<bool> = widths
        .iter()
        .zip(min_widths.iter())
        .map(|(width, min_width)| *width > *min_width)
        .collect();

    while remaining_deficit > 0.01 && flexible.iter().any(|is_flexible| *is_flexible) {
        let flexible_count = flexible.iter().filter(|is_flexible| **is_flexible).count() as f32;
        let shrink_per_column = remaining_deficit / flexible_count;
        let mut consumed_deficit: f32 = 0.0;

        for ((width, min_width), is_flexible) in widths
            .iter_mut()
            .zip(min_widths.iter())
            .zip(flexible.iter_mut())
        {
            if !*is_flexible {
                continue;
            }

            let available_shrink = (*width - *min_width).max(0.0);
            let shrink = available_shrink.min(shrink_per_column);
            *width -= shrink;
            consumed_deficit += shrink;

            if (*width - *min_width).abs() <= 0.01 {
                *is_flexible = false;
            }
        }

        if consumed_deficit <= 0.01 {
            break;
        }
        remaining_deficit -= consumed_deficit;
    }

    normalize_column_widths(&mut widths, target_total_width);
    widths
}

fn normalize_column_widths(widths: &mut [f32], target_total_width: f32) {
    let adjusted_total = widths.iter().sum::<f32>();
    if let Some(last) = widths.last_mut() {
        *last = (*last + target_total_width - adjusted_total).max(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{adaptive_min_column_width, expand_column_widths, fit_column_widths};

    #[test]
    fn expand_column_widths_fills_target_width() {
        let widths = expand_column_widths(vec![140.0, 140.0, 140.0], 840.0);
        let total = widths.iter().sum::<f32>();

        assert!((total - 840.0).abs() < 0.01);
        assert!(widths.iter().all(|width| *width > 140.0));
    }

    #[test]
    fn expand_column_widths_keeps_existing_when_already_wide_enough() {
        let widths = expand_column_widths(vec![200.0, 220.0], 300.0);
        assert_eq!(widths, vec![200.0, 220.0]);
    }

    #[test]
    fn fit_column_widths_shrinks_to_target_width() {
        let widths = fit_column_widths(vec![240.0, 320.0, 440.0], 600.0);
        let total = widths.iter().sum::<f32>();

        assert!((total - 600.0).abs() < 0.01);
        assert!(widths.iter().all(|width| *width >= 88.0));
    }

    #[test]
    fn fit_column_widths_compresses_many_columns_to_viewport() {
        let widths = fit_column_widths(vec![140.0; 10], 620.0);
        let total = widths.iter().sum::<f32>();

        assert!((total - 620.0).abs() < 0.01);
        assert!(widths.iter().all(|width| *width >= 1.0));
    }

    #[test]
    fn adaptive_min_column_width_allows_narrow_many_column_tables() {
        assert!(adaptive_min_column_width(8, 480.0) < 88.0);
    }
}
