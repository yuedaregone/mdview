//! Text selection and clipboard copy
//!
//! Simplified text selection for a read-only markdown viewer.
//! Collects text segment positions during rendering, supports mouse drag selection,
//! and copies selected text to clipboard via arboard.

use egui::{Pos2, Rect, Response, Ui};

/// A segment of rendered text with its screen position
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TextSegment {
    /// Screen-space rectangle of this text segment
    pub rect: Rect,
    /// Plain text content
    pub text: String,
    /// Index of the block this segment belongs to
    pub block_index: usize,
}

/// Text selection state
#[derive(Debug, Clone)]
pub struct TextSelector {
    /// Whether the user is currently dragging to select
    pub selecting: bool,
    /// Selection start position (screen coordinates)
    pub start: Option<Pos2>,
    /// Selection end position (screen coordinates)
    pub end: Option<Pos2>,
    /// Currently selected text
    pub selected_text: String,
    /// All text segments collected during the current frame's rendering
    segments: Vec<TextSegment>,
}

impl TextSelector {
    pub fn new() -> Self {
        Self {
            selecting: false,
            start: None,
            end: None,
            selected_text: String::new(),
            segments: Vec::new(),
        }
    }

    /// Clear all segments at the start of a new frame
    pub fn clear_segments(&mut self) {
        self.segments.clear();
    }

    /// Register a text segment during rendering
    pub fn add_segment(&mut self, rect: Rect, text: String, block_index: usize) {
        if !text.is_empty() && rect.width() > 0.0 {
            self.segments.push(TextSegment {
                rect,
                text,
                block_index,
            });
        }
    }

    /// Handle mouse input for selection. Call this after rendering.
    pub fn handle_input(&mut self, response: &Response) {
        // Start selection on left mouse button press
        if response.drag_started() && response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.hover_pos() {
                self.selecting = true;
                self.start = Some(pos);
                self.end = Some(pos);
                self.selected_text.clear();
            }
        }

        // Update selection while dragging
        if self.selecting && response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.hover_pos() {
                self.end = Some(pos);
                self.update_selected_text();
            }
        }

        // End selection on mouse release
        if response.drag_stopped() {
            self.selecting = false;
        }

        // Click without drag = clear selection
        if response.clicked_by(egui::PointerButton::Primary) {
            self.start = None;
            self.end = None;
            self.selected_text.clear();
        }
    }

    /// Update the selected text based on start/end positions
    fn update_selected_text(&mut self) {
        let (start, end) = match (self.start, self.end) {
            (Some(s), Some(e)) => {
                // Normalize so start is always top-left of end
                if s.y < e.y || (s.y == e.y && s.x <= e.x) {
                    (s, e)
                } else {
                    (e, s)
                }
            }
            _ => return,
        };

        let mut selected = String::new();
        for seg in &self.segments {
            // Check if this segment overlaps with the selection rectangle
            if seg.rect.top() > end.y || seg.rect.bottom() < start.y {
                continue;
            }

            // For segments in the same row, check horizontal overlap
            let in_start_row = seg.rect.top() <= start.y && seg.rect.bottom() >= start.y;
            let in_end_row = seg.rect.top() <= end.y && seg.rect.bottom() >= end.y;

            let overlaps = if in_start_row && in_end_row {
                // Same row — check horizontal range
                seg.rect.right() >= start.x && seg.rect.left() <= end.x
            } else if in_start_row {
                // Start row — must be to the right of start
                seg.rect.right() >= start.x
            } else if in_end_row {
                // End row — must be to the left of end
                seg.rect.left() <= end.x
            } else {
                // Middle row — fully selected
                true
            };

            if overlaps {
                if !selected.is_empty() {
                    selected.push(' ');
                }
                selected.push_str(&seg.text);
            }
        }

        self.selected_text = selected;
    }

    /// Check if there is any text currently selected
    #[allow(dead_code)]
    pub fn has_selection(&self) -> bool {
        !self.selected_text.is_empty()
    }

    /// Copy the selected text to the system clipboard
    pub fn copy_to_clipboard(&self) {
        if self.selected_text.is_empty() {
            return;
        }
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&self.selected_text);
        }
    }

    /// Draw selection highlights over selected segments
    pub fn draw_selection(&self, ui: &mut Ui, selection_color: egui::Color32) {
        if self.start.is_none() || self.end.is_none() || self.selected_text.is_empty() {
            return;
        }

        let (start, end) = match (self.start, self.end) {
            (Some(s), Some(e)) if s.y < e.y || (s.y == e.y && s.x <= e.x) => (s, e),
            (Some(s), Some(e)) => (e, s),
            _ => return,
        };

        for seg in &self.segments {
            let seg_top = seg.rect.top();
            let seg_bottom = seg.rect.bottom();

            if seg_top > end.y || seg_bottom < start.y {
                continue;
            }

            let in_start_row = seg_top <= start.y && seg_bottom >= start.y;
            let in_end_row = seg_top <= end.y && seg_bottom >= end.y;

            let overlaps = if in_start_row && in_end_row {
                seg.rect.right() >= start.x && seg.rect.left() <= end.x
            } else if in_start_row {
                seg.rect.right() >= start.x
            } else if in_end_row {
                seg.rect.left() <= end.x
            } else {
                true
            };

            if overlaps {
                // Draw selection highlight behind the text
                let mut highlight_rect = seg.rect;
                if in_start_row {
                    highlight_rect.set_left(highlight_rect.left().max(start.x));
                }
                if in_end_row {
                    highlight_rect.set_right(highlight_rect.right().min(end.x));
                }
                ui.painter()
                    .rect_filled(highlight_rect, 0.0, selection_color);
            }
        }
    }
}

impl Default for TextSelector {
    fn default() -> Self {
        Self::new()
    }
}
