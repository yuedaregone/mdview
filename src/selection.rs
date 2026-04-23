//! Text selection and clipboard copy
//!
//! Character-range based selection for a read-only markdown viewer.

use std::sync::Arc;

use egui::{Galley, Pos2, Rect, Response, Ui};

/// A rendered text segment tracked for selection.
#[derive(Clone)]
pub struct TextSegment {
    /// Screen-space rectangle of this text segment.
    pub rect: Rect,
    /// Plain text content.
    pub text: String,
    /// Pre-layout galley used for hit testing character positions.
    pub galley: Arc<Galley>,
    /// Separator inserted before this segment when composing copied text.
    pub separator_before: &'static str,
}

#[derive(Clone, Copy)]
struct TextBoundary {
    segment_index: usize,
    char_index: usize,
}

/// Text selection state.
#[derive(Default, Clone)]
pub struct TextSelector {
    /// Currently selected text.
    pub selected_text: String,
    /// All text segments collected during the current frame's rendering.
    segments: Vec<TextSegment>,
    /// Whether currently dragging to select.
    is_selecting: bool,
    /// Start position of selection drag.
    selection_start: Option<Pos2>,
    /// Current end position of selection drag.
    selection_end: Option<Pos2>,
}

impl TextSelector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all segments at the start of a new frame.
    pub fn clear_segments(&mut self) {
        self.segments.clear();
    }

    /// Register a text segment during rendering.
    pub fn add_segment(
        &mut self,
        rect: Rect,
        text: String,
        galley: Arc<Galley>,
        separator_before: &'static str,
    ) {
        if !text.is_empty() && rect.width() > 0.0 && rect.height() > 0.0 {
            self.segments.push(TextSegment {
                rect,
                text,
                galley,
                separator_before,
            });
        }
    }

    /// Check if there is any text currently selected.
    pub fn has_selection(&self) -> bool {
        !self.selected_text.is_empty()
    }

    /// Copy the selected text to the system clipboard.
    pub fn copy_to_clipboard(&self) {
        if self.selected_text.is_empty() {
            return;
        }
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&self.selected_text);
        }
    }

    /// Handle mouse input for text selection.
    pub fn handle_input(&mut self, ui: &Ui, response: &Response) {
        if response.drag_started() {
            self.is_selecting = true;
            self.selection_start = ui.input(|i| i.pointer.press_origin());
            self.selection_end = self.selection_start;
            self.selected_text.clear();
        }

        if self.is_selecting {
            if ui.input(|i| i.pointer.primary_down()) {
                self.selection_end = ui.input(|i| i.pointer.latest_pos());
                self.update_selection();
            } else {
                self.is_selecting = false;
            }
        }
    }

    fn update_selection(&mut self) {
        let Some(start_pos) = self.selection_start else {
            return;
        };
        let Some(end_pos) = self.selection_end else {
            return;
        };

        let Some(start) = self.locate_boundary(start_pos) else {
            self.selected_text.clear();
            return;
        };
        let Some(end) = self.locate_boundary(end_pos) else {
            self.selected_text.clear();
            return;
        };

        let (start, end) = if self.is_boundary_before(start, end) {
            (start, end)
        } else {
            (end, start)
        };

        let mut selected = String::new();
        for idx in start.segment_index..=end.segment_index {
            let segment = &self.segments[idx];
            let segment_char_count = segment.text.chars().count();
            let from = if idx == start.segment_index {
                start.char_index.min(segment_char_count)
            } else {
                0
            };
            let to = if idx == end.segment_index {
                end.char_index.min(segment_char_count)
            } else {
                segment_char_count
            };

            if from == to {
                continue;
            }

            if !selected.is_empty() {
                selected.push_str(segment.separator_before);
            }
            selected.push_str(&slice_chars(&segment.text, from, to));
        }

        self.selected_text = selected;
    }

    fn locate_boundary(&self, pos: Pos2) -> Option<TextBoundary> {
        let (segment_index, segment) =
            self.segments.iter().enumerate().min_by(|(_, a), (_, b)| {
                distance_sq_to_rect(pos, a.rect)
                    .partial_cmp(&distance_sq_to_rect(pos, b.rect))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?;

        let local_pos = clamp_pos_to_rect(pos, segment.rect) - segment.rect.min;
        let cursor = segment.galley.cursor_from_pos(local_pos);
        let char_index = cursor.ccursor.index.min(segment.text.chars().count());

        Some(TextBoundary {
            segment_index,
            char_index,
        })
    }

    fn is_boundary_before(&self, left: TextBoundary, right: TextBoundary) -> bool {
        (left.segment_index, left.char_index) <= (right.segment_index, right.char_index)
    }
}

fn slice_chars(text: &str, from: usize, to: usize) -> String {
    if from >= to {
        return String::new();
    }

    let start = char_to_byte_index(text, from);
    let end = char_to_byte_index(text, to);
    text[start..end].to_string()
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

fn distance_sq_to_rect(pos: Pos2, rect: Rect) -> f32 {
    let dx = if pos.x < rect.left() {
        rect.left() - pos.x
    } else if pos.x > rect.right() {
        pos.x - rect.right()
    } else {
        0.0
    };

    let dy = if pos.y < rect.top() {
        rect.top() - pos.y
    } else if pos.y > rect.bottom() {
        pos.y - rect.bottom()
    } else {
        0.0
    };

    dx * dx + dy * dy
}

fn clamp_pos_to_rect(pos: Pos2, rect: Rect) -> Pos2 {
    Pos2::new(
        pos.x.clamp(rect.left(), rect.right()),
        pos.y.clamp(rect.top(), rect.bottom()),
    )
}

#[cfg(test)]
mod tests {
    use super::{char_to_byte_index, slice_chars};

    #[test]
    fn slice_chars_handles_utf8_boundaries() {
        assert_eq!(slice_chars("中文abc", 0, 2), "中文");
        assert_eq!(slice_chars("中文abc", 2, 5), "abc");
    }

    #[test]
    fn char_to_byte_index_returns_text_len_at_end() {
        let text = "表格";
        assert_eq!(char_to_byte_index(text, 0), 0);
        assert_eq!(char_to_byte_index(text, 1), "表".len());
        assert_eq!(char_to_byte_index(text, 2), text.len());
        assert_eq!(char_to_byte_index(text, 99), text.len());
    }
}
