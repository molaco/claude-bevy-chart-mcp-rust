//! ViewportState - Manages the visible window into chart data.

use bevy::prelude::{Rect, Resource};

/// Viewport state resource - manages the visible window into the data.
///
/// This resource tracks which candles are currently visible, the total
/// chart area, and various state flags for rendering and data loading.
#[derive(Resource)]
pub struct ViewportState {
    /// Index of the first visible candle
    pub visible_candle_start: usize,
    /// Number of candles currently visible
    pub visible_candle_count: usize,
    /// Total chart viewport for resize calculations
    pub total_area: Rect,
    /// True when chart needs to be redrawn
    pub needs_redraw: bool,
    /// True when fetching more data (lazy loading in progress)
    pub loading: bool,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            visible_candle_start: 0,
            visible_candle_count: crate::config::DEFAULT_VISIBLE_CANDLES,
            total_area: Rect::default(),
            needs_redraw: true,
            loading: false,
        }
    }
}

impl ViewportState {
    /// Create a new viewport state with specified parameters.
    pub fn new(visible_candle_start: usize, visible_candle_count: usize, total_area: Rect) -> Self {
        Self {
            visible_candle_start,
            visible_candle_count,
            total_area,
            needs_redraw: true,
            loading: false,
        }
    }

    /// Get the end index of the visible range (exclusive).
    pub fn visible_candle_end(&self) -> usize {
        self.visible_candle_start + self.visible_candle_count
    }

    /// Check if a candle index is currently visible.
    pub fn is_candle_visible(&self, index: usize) -> bool {
        index >= self.visible_candle_start && index < self.visible_candle_end()
    }

    /// Request a redraw on the next frame.
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }
}
