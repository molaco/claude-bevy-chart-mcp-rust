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
