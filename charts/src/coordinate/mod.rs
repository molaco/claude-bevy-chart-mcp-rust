//! Coordinate module - Coordinate transformation and viewport management.
//!
//! This module provides coordinate systems for mapping between chart data
//! (candle indices, prices) and screen coordinates (Bevy world space).
//!
//! # Contents
//!
//! - [`ChartSpace`]: Coordinate space for a single pane
//! - [`ViewportState`]: Bevy resource managing visible window into data
//! - [`right_spacing_candles`]: Calculate right-edge padding

mod chart_space;
mod viewport;

pub use chart_space::ChartSpace;
pub use viewport::ViewportState;

/// Calculate number of "virtual candles" worth of space to add on the right edge.
///
/// This creates empty space when viewing the latest/newest candles,
/// allowing the user to see the most recent data without it being pressed
/// against the right edge.
///
/// Returns approximately 1/3 of the visible candle count.
pub fn right_spacing_candles(visible_candle_count: usize) -> usize {
    visible_candle_count / 3
}
