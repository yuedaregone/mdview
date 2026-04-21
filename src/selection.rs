//! Text selection and clipboard copy
//!
//! Simplified text selection for a read-only markdown viewer.
//! Collects text segment positions during rendering, supports mouse drag selection,
//! and copies selected text to clipboard via arboard.

use egui::{pos2, Context, Pos2, Rect};

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
    pub fn add_segment(&mut self, rect: Rect, text: String, block_id: egui::Id) {
        if !text.is_empty() && rect.width() > 0.0 {
            self.segments.push(TextSegment {
                rect,
                text,
                block_id,
            });
        }
    }

    /// Handle mouse input for selection using raw events.
    /// Takes scroll_offset to convert screen coordinates to document coordinates.
    pub fn handle_input_raw(&mut self, ctx: &Context, scroll_offset: f32) {
        ctx.input(|input| {
            let pointer = &input.pointer;

            if pointer.primary_down() {
                if let Some(pos) = pointer.interact_pos() {
                    // 关键修复：加上 scroll_offset 转换为文档坐标
                    let doc_pos = pos2(pos.x, pos.y + scroll_offset);
                    if !self.selecting {
                        self.selecting = true;
                        self.start = Some(doc_pos);
                        self.end = Some(doc_pos);
                        self.selected_text.clear();
                    } else {
                        self.end = Some(doc_pos);
                        self.update_selected_text();
                    }
                }
            } else {
                self.selecting = false;
            }
        });
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
        let mut last_block_id: Option<egui::Id> = None;

        for seg in &self.segments {
            // 垂直方向快速剔除
            if seg.rect.top() > end.y || seg.rect.bottom() < start.y {
                continue;
            }

            let in_start_row = seg.rect.top() <= start.y && seg.rect.bottom() >= start.y;
            let in_end_row = seg.rect.top() <= end.y && seg.rect.bottom() >= end.y;

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
                // 只在不同 block 之间添加空格，同一 block 内保持原文
                if !selected.is_empty() && Some(seg.block_id) != last_block_id {
                    selected.push(' ');
                }
                selected.push_str(&seg.text);
                last_block_id = Some(seg.block_id);
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
}

impl Default for TextSelector {
    fn default() -> Self {
        Self::new()
    }
}
