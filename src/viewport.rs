//! Viewport state for rendering dimensions
//!
//! Tracks scroll position and viewport dimensions.

const ESTIMATED_LINE_HEIGHT: f32 = 60.0;

#[derive(Debug, Clone)]
pub struct BlockLayout {
    pub height: f32,
    pub measured: bool,
}

#[derive(Debug, Clone)]
pub struct ViewportState {
    pub blocks: Vec<BlockLayout>,
    pub initialized: bool,
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
            initialized: false,
        }
    }
}
