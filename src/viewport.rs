//! Viewport state for rendering dimensions
//!
//! Tracks scroll position and viewport dimensions.

const ESTIMATED_LINE_HEIGHT: f32 = 20.0;

#[derive(Debug, Clone)]
pub struct BlockLayout {
    pub height: f32,
    pub measured: bool,
}

#[derive(Debug, Clone)]
pub struct ViewportState {
    pub blocks: Vec<BlockLayout>,
    pub scroll_offset: f32,
    pub viewport_height: f32,
}

impl ViewportState {
    pub fn new(block_count: usize) -> Self {
        Self {
            blocks: (0..block_count)
                .map(|_| BlockLayout {
                    height: ESTIMATED_LINE_HEIGHT,
                    measured: false,
                })
                .collect(),
            scroll_offset: 0.0,
            viewport_height: 700.0,
        }
    }
}
