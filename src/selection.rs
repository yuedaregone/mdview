//! Text selection and clipboard copy
//!
//! Simplified text selection for a read-only markdown viewer.
//! Collects text segment positions during rendering, supports mouse drag selection,
//! and copies selected text to clipboard via arboard.

use egui::{Pos2, Rect, Response, Ui};

/// A segment of rendered text with its screen position
#[derive(Debug, Clone)]
pub struct TextSegment {
    /// Screen-space rectangle of this text segment
    pub rect: Rect,
    /// Plain text content
    pub text: String,
}

/// Text selection state
#[derive(Debug, Clone)]
pub struct TextSelector {
    /// Currently selected text
    pub selected_text: String,
    /// All text segments collected during the current frame's rendering
    segments: Vec<TextSegment>,
    /// Whether currently dragging to select
    is_selecting: bool,
    /// Start position of selection drag
    selection_start: Option<Pos2>,
    /// Current end position of selection drag
    selection_end: Option<Pos2>,
}

impl TextSelector {
    pub fn new() -> Self {
        Self {
            selected_text: String::new(),
            segments: Vec::new(),
            is_selecting: false,
            selection_start: None,
            selection_end: None,
        }
    }

    /// Clear all segments at the start of a new frame
    pub fn clear_segments(&mut self) {
        self.segments.clear();
    }

    /// Register a text segment during rendering
    pub fn add_segment(&mut self, rect: Rect, text: String) {
        if !text.is_empty() && rect.width() > 0.0 {
            self.segments.push(TextSegment { rect, text });
        }
    }

    /// Check if there is any text currently selected
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

    /// Handle mouse input for text selection
    pub fn handle_input(&mut self, ui: &Ui, response: &Response) {
        if response.drag_started() {
            self.is_selecting = true;
            self.selection_start = ui.input(|i| i.pointer.press_origin());
            self.selection_end = self.selection_start;
            self.selected_text.clear();
        }

        if self.is_selecting {
            if response.dragged() {
                self.selection_end = ui.input(|i| i.pointer.latest_pos());
                self.update_selection();
            }
            if response.drag_stopped() {
                self.is_selecting = false;
            }
        }
    }

    /// Update selected text based on current selection rectangle
    fn update_selection(&mut self) {
        let start = match self.selection_start {
            Some(s) => s,
            None => return,
        };
        let end = match self.selection_end {
            Some(e) => e,
            None => return,
        };

        let selection_rect = Rect::from_two_pos(start, end);

        // Collect all text within the selection rectangle
        let mut selected = String::new();
        for segment in &self.segments {
            if selection_rect.intersects(segment.rect) {
                if !selected.is_empty() {
                    selected.push(' ');
                }
                selected.push_str(&segment.text);
            }
        }

        self.selected_text = selected;
    }
}

impl Default for TextSelector {
    fn default() -> Self {
        Self::new()
    }
}
