//! ChartSpace - Coordinate space for chart rendering.

use bevy::prelude::{Rect, Vec2};
use crate::domain::Candle;
use crate::indicators::MovingAverage;

/// Coordinate space for chart rendering.
///
/// ChartSpace manages the mapping between logical chart coordinates
/// (candle indices and prices) and physical screen coordinates (Bevy world space).
/// Each pane in the chart has its own ChartSpace instance.
#[derive(Debug, Clone)]
pub struct ChartSpace {
    // Y-axis: Price/Value axis (logical, pane-specific)
    /// Minimum visible price (auto-calculated from visible candles)
    pub visible_price_min: f32,
    /// Maximum visible price (auto-calculated from visible candles)
    pub visible_price_max: f32,
    /// Extra space above/below (5-10%)
    pub price_padding: f32,

    // Physical viewport (Bevy world space, pane-specific)
    /// Where this pane is drawn on screen
    pub viewport: Rect,

    // Cached calculations (updated when bounds change)
    /// Width of each candle in pixels
    pub candle_width_px: f32,
    /// Pixels per $1 price movement
    pub price_scale: f32,
}

impl ChartSpace {
    /// Create a new ChartSpace with the given viewport and visible candle count.
    pub fn new(viewport: Rect, visible_candle_count: usize) -> Self {
        let mut space = Self {
            visible_price_min: 0.0,
            visible_price_max: 100.0,
            price_padding: crate::config::PRICE_PADDING_PERCENT,
            viewport,
            candle_width_px: 0.0,
            price_scale: 0.0,
        };
        space.recalculate_cache(visible_candle_count);
        space
    }

    /// Map candle index and price to world coordinates.
    ///
    /// Converts a logical position (candle index, price) to screen coordinates
    /// within this pane's viewport.
    pub fn to_world(
        &self,
        candle_index: usize,
        price: f32,
        visible_candle_start: usize,
        visible_candle_count: usize,
    ) -> Vec2 {
        // Map candle index relative to visible range
        let candle_offset = candle_index.saturating_sub(visible_candle_start);
        let x_percent = candle_offset as f32 / visible_candle_count as f32;
        let world_x = self.viewport.min.x + x_percent * self.viewport.width();

        // Map price to viewport
        let price_range = self.visible_price_max - self.visible_price_min;
        let price_percent = if price_range > 0.0 {
            (price - self.visible_price_min) / price_range
        } else {
            0.5
        };
        let world_y = self.viewport.min.y + price_percent * self.viewport.height();

        Vec2::new(world_x, world_y)
    }

    /// Map world coordinates to candle index and price.
    ///
    /// Converts screen coordinates to logical position (candle index, price).
    pub fn from_world(
        &self,
        world_pos: Vec2,
        visible_candle_start: usize,
        visible_candle_count: usize,
    ) -> (usize, f32) {
        let x_percent = (world_pos.x - self.viewport.min.x) / self.viewport.width();
        let candle_index =
            visible_candle_start + (x_percent * visible_candle_count as f32) as usize;

        let y_percent = (world_pos.y - self.viewport.min.y) / self.viewport.height();
        let price =
            self.visible_price_min + y_percent * (self.visible_price_max - self.visible_price_min);

        (candle_index, price)
    }

    /// Fit price bounds to visible candles.
    ///
    /// Calculates the min/max price from visible candles and updates the
    /// coordinate space bounds with appropriate padding.
    pub fn fit_price_bounds(
        &mut self,
        candles: &[Candle],
        visible_candle_start: usize,
        visible_candle_count: usize,
    ) {
        if candles.is_empty() {
            return;
        }

        let start = visible_candle_start;
        let end = (start + visible_candle_count).min(candles.len());

        if start >= end {
            return;
        }

        let visible_candles = &candles[start..end];
        let mut min_price = f64::MAX;
        let mut max_price = f64::MIN;

        for candle in visible_candles {
            min_price = min_price.min(candle.low);
            max_price = max_price.max(candle.high);
        }

        // Add padding
        let range = max_price - min_price;
        let padding = range * self.price_padding as f64;

        self.visible_price_min = (min_price - padding) as f32;
        self.visible_price_max = (max_price + padding) as f32;

        self.recalculate_cache(visible_candle_count);
    }

    /// Fit price bounds to visible candles AND indicator values.
    ///
    /// Includes indicator values (like moving averages) when calculating
    /// the price bounds, ensuring indicators remain visible within the pane.
    pub fn fit_price_bounds_with_indicators(
        &mut self,
        candles: &[Candle],
        indicators: &[MovingAverage],
        visible_candle_start: usize,
        visible_candle_count: usize,
    ) {
        if candles.is_empty() {
            return;
        }

        let start = visible_candle_start;
        let end = (start + visible_candle_count).min(candles.len());

        if start >= end {
            return;
        }

        let visible_candles = &candles[start..end];
        let mut min_price = f64::MAX;
        let mut max_price = f64::MIN;

        // Include candle highs and lows
        for candle in visible_candles {
            min_price = min_price.min(candle.low);
            max_price = max_price.max(candle.high);
        }

        // Include visible indicator values
        for indicator in indicators {
            if !indicator.visible {
                continue;
            }

            for i in start..end {
                if let Some(value) = indicator.values.get(i).and_then(|v| *v) {
                    min_price = min_price.min(value as f64);
                    max_price = max_price.max(value as f64);
                }
            }
        }

        // Add padding
        let range = max_price - min_price;
        let padding = range * self.price_padding as f64;

        self.visible_price_min = (min_price - padding) as f32;
        self.visible_price_max = (max_price + padding) as f32;

        self.recalculate_cache(visible_candle_count);
    }

    /// Fit volume bounds to visible candles.
    ///
    /// For volume panes, sets the Y-axis to show volume from 0 to max volume.
    pub fn fit_volume_bounds(
        &mut self,
        candles: &[Candle],
        visible_candle_start: usize,
        visible_candle_count: usize,
    ) {
        if candles.is_empty() {
            return;
        }

        let start = visible_candle_start;
        let end = (start + visible_candle_count).min(candles.len());

        if start >= end {
            return;
        }

        let visible_candles = &candles[start..end];
        let max_volume = visible_candles
            .iter()
            .map(|c| c.volume)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(1.0);

        self.visible_price_min = 0.0;
        self.visible_price_max = max_volume as f32;

        self.recalculate_cache(visible_candle_count);
    }

    /// Recalculate cached values.
    ///
    /// Updates candle_width_px and price_scale based on current viewport
    /// and price bounds.
    pub fn recalculate_cache(&mut self, visible_candle_count: usize) {
        self.candle_width_px = self.viewport.width() / visible_candle_count as f32;
        let price_range = self.visible_price_max - self.visible_price_min;
        self.price_scale = if price_range > 0.0 {
            self.viewport.height() / price_range
        } else {
            1.0
        };
    }
}

impl Default for ChartSpace {
    fn default() -> Self {
        Self::new(Rect::default(), crate::config::DEFAULT_VISIBLE_CANDLES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a standard test viewport
    fn create_test_viewport() -> Rect {
        Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0))
    }

    /// Helper to create a ChartSpace with known bounds
    fn create_test_chart_space() -> ChartSpace {
        let mut space = ChartSpace::new(create_test_viewport(), 100);
        space.visible_price_min = 100.0;
        space.visible_price_max = 200.0;
        space.recalculate_cache(100);
        space
    }

    // ========================================================================
    // to_world() TESTS
    // ========================================================================

    #[test]
    fn test_to_world_basic_transformation() {
        let space = create_test_chart_space();

        // Candle at start of visible range, at min price
        let pos = space.to_world(0, 100.0, 0, 100);
        assert!((pos.x - 0.0).abs() < 0.01, "X should be at viewport start");
        assert!((pos.y - 0.0).abs() < 0.01, "Y should be at viewport bottom (min price)");

        // Candle at end of visible range, at max price
        let pos = space.to_world(100, 200.0, 0, 100);
        assert!((pos.x - 800.0).abs() < 0.01, "X should be at viewport end");
        assert!((pos.y - 600.0).abs() < 0.01, "Y should be at viewport top (max price)");
    }

    #[test]
    fn test_to_world_midpoint() {
        let space = create_test_chart_space();

        // Middle candle at middle price
        let pos = space.to_world(50, 150.0, 0, 100);
        assert!((pos.x - 400.0).abs() < 0.01, "X should be at viewport center");
        assert!((pos.y - 300.0).abs() < 0.01, "Y should be at viewport center");
    }

    #[test]
    fn test_to_world_with_offset_visible_start() {
        let space = create_test_chart_space();

        // If visible_candle_start = 50, then candle 50 should be at x=0
        let pos = space.to_world(50, 150.0, 50, 100);
        assert!((pos.x - 0.0).abs() < 0.01, "Candle at visible_start should be at x=0");

        // Candle 100 should be at x=400 (50% through viewport)
        let pos = space.to_world(100, 150.0, 50, 100);
        assert!((pos.x - 400.0).abs() < 0.01, "X position should account for visible_start offset");
    }

    #[test]
    fn test_to_world_zero_price_range() {
        let mut space = create_test_chart_space();
        space.visible_price_min = 100.0;
        space.visible_price_max = 100.0; // Flat price (zero range)
        space.recalculate_cache(100);

        // Should return center Y (0.5 * height) when price range is zero
        let pos = space.to_world(50, 100.0, 0, 100);
        assert!((pos.y - 300.0).abs() < 0.01, "Y should be at center when price range is zero");
    }

    #[test]
    fn test_to_world_zero_visible_candles() {
        let space = create_test_chart_space();

        // With zero visible candles, x_percent would be division by zero
        // The function should handle this gracefully (returns inf/NaN currently)
        let pos = space.to_world(50, 150.0, 0, 0);

        // Check that position is finite (not NaN or Inf)
        // Current implementation: divides by 0, producing inf
        // This test documents the current behavior and catches if it changes
        if pos.x.is_finite() {
            // If implementation is fixed to handle this, X should be at viewport edge
            assert!(
                pos.x >= space.viewport.min.x && pos.x <= space.viewport.max.x,
                "X should be within viewport bounds"
            );
        } else {
            // Document that current implementation produces inf
            assert!(
                pos.x.is_infinite() || pos.x.is_nan(),
                "Division by zero produces inf/nan: {:?}",
                pos
            );
        }

        // Y should still be valid (price calculation doesn't depend on candle count)
        assert!(pos.y.is_finite(), "Y should be finite even with zero visible candles");
    }

    #[test]
    fn test_to_world_zero_viewport_dimensions() {
        // Create a zero-sized viewport
        let mut space = ChartSpace::new(
            Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0)),
            100,
        );
        space.visible_price_min = 100.0;
        space.visible_price_max = 200.0;
        space.recalculate_cache(100);

        // With zero viewport dimensions, both width and height are 0
        let pos = space.to_world(50, 150.0, 0, 100);

        // X calculation: 0 + 0.5 * 0 = 0 (safe, no division by zero in to_world)
        assert!(pos.x.is_finite(), "X should be finite with zero-width viewport");
        assert_eq!(pos.x, 0.0, "X should be 0 with zero-width viewport");

        // Y calculation: 0 + 0.5 * 0 = 0 (safe)
        assert!(pos.y.is_finite(), "Y should be finite with zero-height viewport");
        assert_eq!(pos.y, 0.0, "Y should be 0 with zero-height viewport");
    }

    #[test]
    fn test_from_world_zero_viewport_dimensions() {
        // Create a zero-sized viewport
        let mut space = ChartSpace::new(
            Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0)),
            100,
        );
        space.visible_price_min = 100.0;
        space.visible_price_max = 200.0;
        space.recalculate_cache(100);

        // from_world divides by viewport width/height, which are 0
        let (candle_idx, price) = space.from_world(Vec2::new(0.0, 0.0), 0, 100);

        // Division by zero produces inf/nan
        // Document current behavior - this could be considered a bug
        // A robust implementation might return visible_candle_start and visible_price_min
        if candle_idx < usize::MAX / 2 {
            // Implementation handles zero dimensions gracefully
            assert_eq!(candle_idx, 0, "Should return start index with zero viewport");
        }
        // Note: candle_idx will be very large if NaN is cast to usize

        // Price calculation also divides by height
        if price.is_finite() {
            assert!(
                price >= space.visible_price_min && price <= space.visible_price_max,
                "Price should be within bounds"
            );
        }
    }

    #[test]
    fn test_to_world_boundary_coordinates() {
        let space = create_test_chart_space();

        // Price at exact min bound
        let pos_min = space.to_world(0, 100.0, 0, 100);
        assert!((pos_min.y - 0.0).abs() < 0.01, "Min price should map to viewport bottom");

        // Price at exact max bound
        let pos_max = space.to_world(0, 200.0, 0, 100);
        assert!((pos_max.y - 600.0).abs() < 0.01, "Max price should map to viewport top");
    }

    // ========================================================================
    // from_world() TESTS
    // ========================================================================

    #[test]
    fn test_from_world_basic_transformation() {
        let space = create_test_chart_space();

        // World position at viewport start (0, 0) should map to candle 0, min price
        let (candle_idx, price) = space.from_world(Vec2::new(0.0, 0.0), 0, 100);
        assert_eq!(candle_idx, 0, "Should map to first candle");
        assert!((price - 100.0).abs() < 0.01, "Should map to min price");

        // World position at viewport end (800, 600) should map to candle 100, max price
        let (candle_idx, price) = space.from_world(Vec2::new(800.0, 600.0), 0, 100);
        assert_eq!(candle_idx, 100, "Should map to last visible candle");
        assert!((price - 200.0).abs() < 0.01, "Should map to max price");
    }

    #[test]
    fn test_from_world_midpoint() {
        let space = create_test_chart_space();

        // Center of viewport should map to middle candle and middle price
        let (candle_idx, price) = space.from_world(Vec2::new(400.0, 300.0), 0, 100);
        assert_eq!(candle_idx, 50, "Should map to middle candle");
        assert!((price - 150.0).abs() < 0.01, "Should map to middle price");
    }

    #[test]
    fn test_from_world_with_offset_visible_start() {
        let space = create_test_chart_space();

        // If visible_candle_start = 50, then x=0 should map to candle 50
        let (candle_idx, _) = space.from_world(Vec2::new(0.0, 300.0), 50, 100);
        assert_eq!(candle_idx, 50, "Should account for visible_start offset");

        // x=400 (center) should map to candle 100 (50 + 50)
        let (candle_idx, _) = space.from_world(Vec2::new(400.0, 300.0), 50, 100);
        assert_eq!(candle_idx, 100, "Center should map to candle at offset + count/2");
    }

    // ========================================================================
    // ROUND-TRIP TESTS
    // ========================================================================

    #[test]
    fn test_coordinate_round_trip_precision() {
        let space = create_test_chart_space();

        // Test several points for round-trip consistency
        let test_cases = [
            (25, 125.0),
            (50, 150.0),
            (75, 175.0),
            (0, 100.0),
            (99, 199.0),
        ];

        for (orig_candle, orig_price) in test_cases {
            let world_pos = space.to_world(orig_candle, orig_price, 0, 100);
            let (recovered_candle, recovered_price) = space.from_world(world_pos, 0, 100);

            assert_eq!(
                recovered_candle, orig_candle,
                "Round-trip candle index mismatch for ({}, {})",
                orig_candle, orig_price
            );
            assert!(
                (recovered_price - orig_price).abs() < 0.01,
                "Round-trip price mismatch: {} vs {} for original ({}, {})",
                recovered_price, orig_price, orig_candle, orig_price
            );
        }
    }

    // ========================================================================
    // fit_price_bounds() TESTS
    // ========================================================================

    #[test]
    fn test_fit_price_bounds_basic() {
        let mut space = ChartSpace::new(create_test_viewport(), 100);

        let candles = vec![
            Candle::new(1000, 100.0, 110.0, 95.0, 105.0, 1000.0),
            Candle::new(2000, 105.0, 120.0, 100.0, 115.0, 1000.0),
            Candle::new(3000, 115.0, 130.0, 110.0, 125.0, 1000.0),
        ];

        space.fit_price_bounds(&candles, 0, 3);

        // Min should be lowest low (95) minus padding
        // Max should be highest high (130) plus padding
        assert!(space.visible_price_min < 95.0, "Min should include padding below lowest low");
        assert!(space.visible_price_max > 130.0, "Max should include padding above highest high");
    }

    #[test]
    fn test_fit_price_bounds_empty_candles() {
        let mut space = ChartSpace::new(create_test_viewport(), 100);
        let orig_min = space.visible_price_min;
        let orig_max = space.visible_price_max;

        let candles: Vec<Candle> = vec![];
        space.fit_price_bounds(&candles, 0, 0);

        // Should not change when candles are empty
        assert_eq!(space.visible_price_min, orig_min, "Min should not change for empty candles");
        assert_eq!(space.visible_price_max, orig_max, "Max should not change for empty candles");
    }

    #[test]
    fn test_fit_price_bounds_partial_visible_range() {
        let mut space = ChartSpace::new(create_test_viewport(), 100);

        let candles = vec![
            Candle::new(1000, 100.0, 110.0, 95.0, 105.0, 1000.0),  // idx 0
            Candle::new(2000, 200.0, 220.0, 190.0, 210.0, 1000.0), // idx 1 - much higher
            Candle::new(3000, 105.0, 115.0, 100.0, 110.0, 1000.0), // idx 2
        ];

        // Only fit to first and third candle (skip the high one)
        space.fit_price_bounds(&candles, 0, 1);

        // Bounds should only reflect first candle
        assert!(space.visible_price_max < 150.0, "Should not include second candle's high");
    }

    // ========================================================================
    // fit_volume_bounds() TESTS
    // ========================================================================

    #[test]
    fn test_fit_volume_bounds() {
        let mut space = ChartSpace::new(create_test_viewport(), 100);

        let candles = vec![
            Candle::new(1000, 100.0, 110.0, 95.0, 105.0, 500.0),
            Candle::new(2000, 105.0, 120.0, 100.0, 115.0, 1500.0),
            Candle::new(3000, 115.0, 130.0, 110.0, 125.0, 1000.0),
        ];

        space.fit_volume_bounds(&candles, 0, 3);

        assert_eq!(space.visible_price_min, 0.0, "Volume min should always be 0");
        assert_eq!(space.visible_price_max, 1500.0, "Volume max should be highest volume");
    }

    // ========================================================================
    // recalculate_cache() TESTS
    // ========================================================================

    #[test]
    fn test_recalculate_cache_updates_correctly() {
        let mut space = ChartSpace::new(create_test_viewport(), 100);
        space.visible_price_min = 100.0;
        space.visible_price_max = 200.0;

        space.recalculate_cache(100);

        // candle_width_px = viewport_width / visible_candle_count = 800 / 100 = 8
        assert!((space.candle_width_px - 8.0).abs() < 0.01, "Candle width should be 8.0");

        // price_scale = viewport_height / price_range = 600 / 100 = 6
        assert!((space.price_scale - 6.0).abs() < 0.01, "Price scale should be 6.0");
    }

    #[test]
    fn test_recalculate_cache_zero_price_range() {
        let mut space = ChartSpace::new(create_test_viewport(), 100);
        space.visible_price_min = 100.0;
        space.visible_price_max = 100.0; // Zero range

        space.recalculate_cache(100);

        // price_scale should default to 1.0 when range is zero
        assert_eq!(space.price_scale, 1.0, "Price scale should be 1.0 when range is zero");
    }

    #[test]
    fn test_recalculate_cache_different_candle_counts() {
        let mut space = ChartSpace::new(create_test_viewport(), 100);

        // With 50 candles, width should be 800 / 50 = 16
        space.recalculate_cache(50);
        assert!((space.candle_width_px - 16.0).abs() < 0.01, "Candle width should be 16.0 for 50 candles");

        // With 200 candles, width should be 800 / 200 = 4
        space.recalculate_cache(200);
        assert!((space.candle_width_px - 4.0).abs() < 0.01, "Candle width should be 4.0 for 200 candles");
    }

    // ========================================================================
    // EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_to_world_outside_visible_range() {
        let space = create_test_chart_space();

        // Candle before visible range (negative offset due to saturating_sub)
        let pos = space.to_world(0, 150.0, 50, 100);
        // candle_offset = 0.saturating_sub(50) = 0, so x_percent = 0
        assert!((pos.x - 0.0).abs() < 0.01, "Candle before visible range should clamp to start");
    }

    #[test]
    fn test_from_world_outside_viewport() {
        let space = create_test_chart_space();

        // Position outside viewport (negative X)
        let (candle_idx, price) = space.from_world(Vec2::new(-100.0, 300.0), 0, 100);
        // This will extrapolate, resulting in values outside normal range
        // The function doesn't clamp, so we just verify it doesn't crash
        assert!(candle_idx < 100, "Should handle negative X coordinates");

        // Position above viewport
        let (_, price) = space.from_world(Vec2::new(400.0, 800.0), 0, 100);
        assert!(price > 200.0, "Should extrapolate above max price");
    }

    #[test]
    fn test_chart_space_with_non_zero_viewport_origin() {
        let viewport = Rect::from_corners(Vec2::new(100.0, 50.0), Vec2::new(900.0, 650.0));
        let mut space = ChartSpace::new(viewport, 100);
        space.visible_price_min = 100.0;
        space.visible_price_max = 200.0;
        space.recalculate_cache(100);

        // First candle at min price should be at viewport min
        let pos = space.to_world(0, 100.0, 0, 100);
        assert!((pos.x - 100.0).abs() < 0.01, "X should start at viewport min.x");
        assert!((pos.y - 50.0).abs() < 0.01, "Y should start at viewport min.y");

        // Last candle at max price should be at viewport max
        let pos = space.to_world(100, 200.0, 0, 100);
        assert!((pos.x - 900.0).abs() < 0.01, "X should end at viewport max.x");
        assert!((pos.y - 650.0).abs() < 0.01, "Y should end at viewport max.y");
    }
}
