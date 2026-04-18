//! Viewport culling for large documents
//!
//! Only renders blocks that are visible in the current viewport,
//! with overscan to avoid pop-in during fast scrolling.
//!
//! Coordinate system: block.y is in **document coordinates** (0 at top, increases downward).
//! The ScrollArea's scroll offset is subtracted to determine what's visible.

/// Estimated line height for blocks that haven't been measured yet
const ESTIMATED_LINE_HEIGHT: f32 = 20.0;

/// Extra pixels above and below the viewport to pre-render
const OVERSCAN_PX: f32 = 500.0;

/// Layout info for a single top-level block
#[derive(Debug, Clone)]
pub struct BlockLayout {
    /// Y position of the top of this block (document coordinates)
    pub y: f32,
    /// Rendered height of this block
    pub height: f32,
    /// Whether the height has been measured via actual rendering
    pub measured: bool,
}

/// Viewport culling state
#[derive(Debug, Clone)]
pub struct ViewportState {
    /// Layout info for each top-level block
    pub blocks: Vec<BlockLayout>,
    /// Total document height
    pub total_height: f32,
    /// Content hash — invalidate when content changes
    pub content_hash: u64,
    /// Current scroll offset from ScrollArea (updated each frame by app.rs)
    pub scroll_offset: f32,
    /// Visible height of the viewport (clip rect height)
    pub viewport_height: f32,
}

impl ViewportState {
    pub fn new(block_count: usize) -> Self {
        Self {
            blocks: (0..block_count)
                .map(|_| BlockLayout {
                    y: 0.0,
                    height: ESTIMATED_LINE_HEIGHT,
                    measured: false,
                })
                .collect(),
            total_height: block_count as f32 * ESTIMATED_LINE_HEIGHT,
            content_hash: 0,
            scroll_offset: 0.0,
            viewport_height: 700.0,
        }
    }

    /// Reset when document changes
    pub fn reset(&mut self, block_count: usize, content_hash: u64) {
        if self.content_hash != content_hash || self.blocks.len() != block_count {
            let old_scroll = self.scroll_offset;
            let old_viewport_h = self.viewport_height;
            *self = Self::new(block_count);
            self.content_hash = content_hash;
            self.scroll_offset = old_scroll;
            self.viewport_height = old_viewport_h;
        }
    }

    /// Estimate heights for unmeasured blocks based on line count
    #[allow(dead_code)]
    pub fn estimate_heights(&mut self, line_counts: &[usize]) {
        for (i, block) in self.blocks.iter_mut().enumerate() {
            if !block.measured {
                if let Some(&lines) = line_counts.get(i) {
                    block.height = (lines as f32 * ESTIMATED_LINE_HEIGHT).max(ESTIMATED_LINE_HEIGHT);
                }
            }
        }
        self.recalc_positions();
    }

    /// Update a block's measured height
    pub fn update_block_height(&mut self, index: usize, height: f32) {
        if let Some(block) = self.blocks.get_mut(index) {
            block.height = height;
            block.measured = true;
        }
        self.recalc_positions();
    }

    /// Recalculate Y positions and total height
    fn recalc_positions(&mut self) {
        let mut y = 0.0;
        for block in &mut self.blocks {
            block.y = y;
            y += block.height + 4.0; // 4px spacing between blocks
        }
        self.total_height = y;
    }

    /// Get the range of block indices that are visible in the current viewport.
    /// Uses scroll_offset and viewport_height to determine the visible document range.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        // Document Y range visible in the viewport
        let doc_top = self.scroll_offset - OVERSCAN_PX;
        let doc_bottom = self.scroll_offset + self.viewport_height + OVERSCAN_PX;

        // Find first visible block (binary search)
        let start = self.blocks.partition_point(|b| b.y + b.height < doc_top);
        // Find last visible block
        let end = self.blocks.partition_point(|b| b.y <= doc_bottom);

        start..end.min(self.blocks.len())
    }

    /// Check if a block should be rendered (within visible range)
    #[allow(dead_code)]
    pub fn is_visible(&self, index: usize) -> bool {
        self.visible_range().contains(&index)
    }
}
