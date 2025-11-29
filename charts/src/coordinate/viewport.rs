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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec2;

    fn create_test_viewport() -> ViewportState {
        ViewportState::new(
            100,  // start at candle 100
            50,   // show 50 candles
            Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0)),
        )
    }

    // ========================================================================
    // CREATION TESTS
    // ========================================================================

    #[test]
    fn test_viewport_state_default() {
        let viewport = ViewportState::default();
        assert_eq!(viewport.visible_candle_start, 0);
        assert_eq!(viewport.visible_candle_count, crate::config::DEFAULT_VISIBLE_CANDLES);
        assert!(viewport.needs_redraw, "Default viewport should need redraw");
        assert!(!viewport.loading, "Default viewport should not be loading");
    }

    #[test]
    fn test_viewport_state_new() {
        let viewport = create_test_viewport();
        assert_eq!(viewport.visible_candle_start, 100);
        assert_eq!(viewport.visible_candle_count, 50);
        assert!(viewport.needs_redraw, "New viewport should need redraw");
        assert!(!viewport.loading, "New viewport should not be loading");
    }

    // ========================================================================
    // visible_candle_end() TESTS
    // ========================================================================

    #[test]
    fn test_visible_candle_end_calculation() {
        let viewport = create_test_viewport();
        // end = start + count = 100 + 50 = 150
        assert_eq!(viewport.visible_candle_end(), 150);
    }

    #[test]
    fn test_visible_candle_end_from_zero() {
        let viewport = ViewportState::new(
            0,
            100,
            Rect::default(),
        );
        assert_eq!(viewport.visible_candle_end(), 100);
    }

    #[test]
    fn test_visible_candle_end_different_ranges() {
        let cases = [
            (0, 10, 10),
            (50, 25, 75),
            (1000, 100, 1100),
            (0, 1, 1),
        ];

        for (start, count, expected_end) in cases {
            let viewport = ViewportState::new(start, count, Rect::default());
            assert_eq!(
                viewport.visible_candle_end(), expected_end,
                "Expected end {} for start={}, count={}", expected_end, start, count
            );
        }
    }

    // ========================================================================
    // is_candle_visible() TESTS
    // ========================================================================

    #[test]
    fn test_is_candle_visible_in_range() {
        let viewport = create_test_viewport();

        // Candles in range [100, 150)
        assert!(viewport.is_candle_visible(100), "First visible candle");
        assert!(viewport.is_candle_visible(125), "Middle candle");
        assert!(viewport.is_candle_visible(149), "Last visible candle");
    }

    #[test]
    fn test_is_candle_visible_outside_range() {
        let viewport = create_test_viewport();

        // Candles outside range
        assert!(!viewport.is_candle_visible(99), "Before visible range");
        assert!(!viewport.is_candle_visible(150), "At end (exclusive)");
        assert!(!viewport.is_candle_visible(200), "After visible range");
        assert!(!viewport.is_candle_visible(0), "Far before range");
    }

    #[test]
    fn test_is_candle_visible_boundary_cases() {
        let viewport = create_test_viewport();

        // Exact boundaries
        assert!(viewport.is_candle_visible(100), "Start boundary (inclusive)");
        assert!(!viewport.is_candle_visible(150), "End boundary (exclusive)");
    }

    #[test]
    fn test_is_candle_visible_single_candle_range() {
        let viewport = ViewportState::new(50, 1, Rect::default());

        assert!(viewport.is_candle_visible(50), "Single candle should be visible");
        assert!(!viewport.is_candle_visible(49), "Before single candle");
        assert!(!viewport.is_candle_visible(51), "After single candle");
    }

    // ========================================================================
    // request_redraw() TESTS
    // ========================================================================

    #[test]
    fn test_request_redraw() {
        let mut viewport = create_test_viewport();
        viewport.needs_redraw = false;

        viewport.request_redraw();

        assert!(viewport.needs_redraw, "Should set needs_redraw to true");
    }

    #[test]
    fn test_request_redraw_idempotent() {
        let mut viewport = create_test_viewport();
        viewport.needs_redraw = true;

        viewport.request_redraw();

        assert!(viewport.needs_redraw, "Should still be true");
    }

    // ========================================================================
    // STATE FLAG TESTS
    // ========================================================================

    #[test]
    fn test_loading_flag() {
        let mut viewport = create_test_viewport();

        assert!(!viewport.loading, "Initially not loading");

        viewport.loading = true;
        assert!(viewport.loading, "Can set loading to true");

        viewport.loading = false;
        assert!(!viewport.loading, "Can set loading to false");
    }

    #[test]
    fn test_needs_redraw_flag() {
        let mut viewport = create_test_viewport();

        assert!(viewport.needs_redraw, "Initially needs redraw");

        viewport.needs_redraw = false;
        assert!(!viewport.needs_redraw, "Can clear needs_redraw");

        viewport.request_redraw();
        assert!(viewport.needs_redraw, "request_redraw sets flag");
    }

    // ========================================================================
    // TOTAL AREA TESTS
    // ========================================================================

    #[test]
    fn test_total_area_stored() {
        let area = Rect::from_corners(Vec2::new(10.0, 20.0), Vec2::new(810.0, 620.0));
        let viewport = ViewportState::new(0, 50, area);

        assert_eq!(viewport.total_area, area);
    }
}
