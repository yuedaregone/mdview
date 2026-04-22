//! Viewport state for rendering dimensions
//!
//! Tracks scroll position and viewport dimensions.

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
    content_width: f32,
    font_size: f32,
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
            content_width: 0.0,
            font_size: 0.0,
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

        needs_reset
    }
}
