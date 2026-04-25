//! Viewport state for rendering dimensions
//!
//! Tracks scroll position and viewport dimensions.

use std::ops::Range;

pub const DEFAULT_BLOCK_HEIGHT: f32 = 60.0;
const LAYOUT_EPSILON: f32 = 0.5;

#[derive(Debug, Clone)]
pub struct BlockLayout {
    pub height: f32,
    pub measured: bool,
}

#[derive(Debug, Clone)]
pub struct ViewportState {
    pub blocks: Vec<BlockLayout>,
    block_offsets: Vec<f32>,
    total_height: f32,
    bottom_padding: f32,
    content_width: f32,
    font_size: f32,
    layout_dirty: bool,
    first_measurement_pass: bool,
}

impl ViewportState {
    pub fn new(block_count: usize) -> Self {
        Self {
            blocks: (0..block_count)
                .map(|_| BlockLayout {
                    height: DEFAULT_BLOCK_HEIGHT,
                    measured: false,
                })
                .collect(),
            block_offsets: Vec::with_capacity(block_count.saturating_add(1)),
            total_height: 0.0,
            bottom_padding: 0.0,
            content_width: 0.0,
            font_size: 0.0,
            layout_dirty: true,
            first_measurement_pass: true,
        }
    }

    pub fn reset(&mut self, block_count: usize) {
        *self = Self::new(block_count);
    }

    pub fn prepare_layout(
        &mut self,
        block_count: usize,
        content_width: f32,
        font_size: f32,
    ) -> bool {
        let needs_reset = self.blocks.len() != block_count
            || (self.content_width - content_width).abs() > LAYOUT_EPSILON
            || (self.font_size - font_size).abs() > f32::EPSILON;

        if needs_reset {
            self.reset(block_count);
        }

        self.content_width = content_width;
        self.font_size = font_size;
        self.layout_dirty |= needs_reset;

        needs_reset
    }

    pub fn mark_layout_dirty(&mut self) {
        self.layout_dirty = true;
    }

    pub fn is_first_measurement_pass(&self) -> bool {
        self.first_measurement_pass
    }

    pub fn finish_measurement_pass(&mut self) {
        self.first_measurement_pass = false;
    }

    pub fn rebuild_positions(&mut self, top_padding: f32, block_spacing: f32, bottom_padding: f32) {
        if !self.layout_dirty && self.block_offsets.len() == self.blocks.len().saturating_add(1) {
            return;
        }

        self.block_offsets.clear();
        self.block_offsets
            .reserve(self.blocks.len().saturating_add(1));

        let mut current_y = top_padding;
        for block in &self.blocks {
            self.block_offsets.push(current_y);
            current_y += block.height + block_spacing;
        }

        self.block_offsets.push(current_y);
        self.total_height = current_y + bottom_padding;
        self.bottom_padding = bottom_padding;
        self.layout_dirty = false;
    }

    pub fn visible_range(&self, vis_top: f32, vis_bottom: f32, overscan: f32) -> Range<usize> {
        if self.blocks.is_empty() {
            return 0..0;
        }

        let visible_top = (vis_top - overscan).max(0.0);
        let visible_bottom = vis_bottom + overscan;

        let start = self.first_block_with_bottom_at_or_after(visible_top);
        let end = self.first_block_with_top_after(visible_bottom);

        start..end.max(start)
    }

    pub fn offset_before(&self, index: usize) -> f32 {
        self.block_offsets
            .get(index)
            .copied()
            .unwrap_or(self.total_height)
    }

    pub fn trailing_space_from(&self, index: usize) -> f32 {
        if index >= self.blocks.len() {
            self.bottom_padding
        } else {
            (self.total_height - self.offset_before(index)).max(0.0)
        }
    }

    pub fn total_height(&self) -> f32 {
        self.total_height
    }

    fn first_block_with_bottom_at_or_after(&self, y: f32) -> usize {
        let mut left = 0usize;
        let mut right = self.blocks.len();

        while left < right {
            let mid = (left + right) / 2;
            let bottom = self.block_offsets[mid] + self.blocks[mid].height;
            if bottom < y {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        left.min(self.blocks.len())
    }

    fn first_block_with_top_after(&self, y: f32) -> usize {
        let mut left = 0usize;
        let mut right = self.blocks.len();

        while left < right {
            let mid = (left + right) / 2;
            if self.block_offsets[mid] <= y {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        left.min(self.blocks.len())
    }
}
