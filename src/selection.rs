//! Text selection and clipboard copy
//!
//! Simplified text selection for a read-only markdown viewer.
//! Collects text segment positions during rendering, supports mouse drag selection,
//! and copies selected text to clipboard via arboard.

use egui::Rect;

/// A segment of rendered text with its screen position
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TextSegment {
    /// Screen-space rectangle of this text segment
    pub rect: Rect,
    /// Plain text content
    pub text: String,
    /// ID of the block this segment belongs to
    pub block_id: egui::Id,
}

/// Text selection state
#[derive(Debug, Clone)]
pub struct TextSelector {
    /// Currently selected text
    pub selected_text: String,
    /// All text segments collected during the current frame's rendering
    segments: Vec<TextSegment>,
}

impl TextSelector {
    pub fn new() -> Self {
        Self {
            selected_text: String::new(),
            segments: Vec::new(),
        }
    }

    /// Clear all segments at the start of a new frame
    pub fn clear_segments(&mut self) {
        self.segments.clear();
    }

    /// Register a text segment during rendering
    pub fn add_segment(&mut self, rect: Rect, text: String, block_id: egui::Id) {
        if !text.is_empty() && rect.width() > 0.0 {
            self.segments.push(TextSegment {
                rect,
                text,
                block_id,
            });
        }
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
}

impl Default for TextSelector {
    fn default() -> Self {
        Self::new()
    }
}
