use bevy::prelude::*;
use duckdb::{params, Connection};
use std::sync::{Arc, Mutex};

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// Raw candle from database
#[derive(Debug, Clone)]
pub struct Candle {
    pub time: i64, // Unix timestamp in ms
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Coordinate space for chart rendering
#[derive(Debug, Clone)]
pub struct ChartSpace {
    // Y-axis: Price/Value axis (logical, pane-specific)
    pub visible_price_min: f32, // Auto-calculated from visible candles
    pub visible_price_max: f32, // Auto-calculated from visible candles
    pub price_padding: f32,     // Extra space above/below (5-10%)

    // Physical viewport (Bevy world space, pane-specific)
    pub viewport: Rect, // Where this pane is drawn on screen

    // Cached calculations (updated when bounds change)
    pub candle_width_px: f32, // Width of each candle in pixels
    pub price_scale: f32,     // Pixels per $1 price movement
}

impl ChartSpace {
    pub fn new(viewport: Rect, visible_candle_count: usize) -> Self {
        let mut space = Self {
            visible_price_min: 0.0,
            visible_price_max: 100.0,
            price_padding: 0.05,
            viewport,
            candle_width_px: 0.0,
            price_scale: 0.0,
        };
        space.recalculate_cache(visible_candle_count);
        space
    }

    /// Map candle index and price to world coordinates
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

    /// Map world coordinates to candle index and price
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

    /// Fit price bounds to visible candles
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

    /// Fit price bounds to visible candles AND indicator values (Option B)
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

    /// Fit volume bounds to visible candles
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

        // Add 12% top padding so volume bars don't touch the pane ceiling
        self.visible_price_min = 0.0;
        self.visible_price_max = (max_volume * 1.12) as f32;

        self.recalculate_cache(visible_candle_count);
    }

    /// Recalculate cached values
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
        Self::new(Rect::default(), 50)
    }
}

/// Pane identifier
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PaneId {
    Price,
    Volume,
    Indicator(usize),
}

/// What type of content this pane renders
#[derive(Clone, Debug)]
pub enum PaneType {
    Price,
    Volume,
    Indicator { name: String },
}

/// Individual pane configuration
#[derive(Clone)]
pub struct Pane {
    pub id: PaneId,
    pub pane_type: PaneType,
    pub height_percent: f32,
    pub space: ChartSpace,
}

impl Pane {
    pub fn new(
        id: PaneId,
        pane_type: PaneType,
        height_percent: f32,
        viewport: Rect,
        visible_candle_count: usize,
    ) -> Self {
        Self {
            id,
            pane_type,
            height_percent,
            space: ChartSpace::new(viewport, visible_candle_count),
        }
    }
}

/// Calculate and assign viewports to each pane based on height percentages
/// Panes are stacked from top to bottom with padding between them
pub fn calculate_pane_layouts(panes: &mut [Pane], total_area: Rect, visible_candle_count: usize) {
    if panes.is_empty() {
        return;
    }

    const SEPARATOR_GAP: f32 = 24.0; // Padding between panes

    // Calculate total height available after accounting for separators
    let num_separators = panes.len().saturating_sub(1);
    let separator_total_height = num_separators as f32 * SEPARATOR_GAP;
    let available_height = total_area.height() - separator_total_height;

    // Start from the top
    let mut current_y = total_area.max.y;
    let num_panes = panes.len();

    for (i, pane) in panes.iter_mut().enumerate() {
        let pane_height = available_height * pane.height_percent;
        let pane_min_y = current_y - pane_height;
        let pane_max_y = current_y;

        // Create viewport for this pane
        // total_area is already sized correctly for the chart viewport
        pane.space.viewport = Rect::from_corners(
            Vec2::new(total_area.min.x, pane_min_y),
            Vec2::new(total_area.max.x, pane_max_y),
        );

        // Recalculate cached values
        pane.space.recalculate_cache(visible_candle_count);

        // Move down for next pane, adding separator gap if not the last pane
        current_y = pane_min_y;
        if i < num_panes - 1 {
            current_y -= SEPARATOR_GAP;
        }
    }
}

/// Moving Average indicator
#[derive(Debug, Clone)]
pub struct MovingAverage {
    pub period: usize,            // 20, 50, 200
    pub values: Vec<Option<f32>>, // Cached MA per candle
    pub name: String,             // "SMA-20"
    pub color: Color,             // Line color
    pub visible: bool,            // Toggle visibility
}

impl MovingAverage {
    /// Calculate Simple Moving Average using sliding window (O(n) complexity)
    pub fn calculate_sma(candles: &[Candle], period: usize) -> Vec<Option<f32>> {
        let mut values = vec![None; candles.len()];

        if candles.len() < period {
            return values;
        }

        // Calculate initial sum for first window [0..period-1]
        let mut sum: f64 = candles[0..period].iter().map(|c| c.close).sum();

        // Store first SMA value
        values[period - 1] = Some((sum / period as f64) as f32);

        // Sliding window: for each subsequent candle
        // Remove oldest value, add newest value
        for i in period..candles.len() {
            sum -= candles[i - period].close; // Remove value leaving window
            sum += candles[i].close; // Add value entering window
            values[i] = Some((sum / period as f64) as f32);
        }

        values
    }

    /// Calculate Simple Moving Average (NAIVE - for testing only)
    #[cfg(test)]
    pub fn calculate_sma_naive(candles: &[Candle], period: usize) -> Vec<Option<f32>> {
        let mut values = vec![None; candles.len()];

        if candles.len() < period {
            return values;
        }

        for i in (period - 1)..candles.len() {
            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();
            values[i] = Some((sum / period as f64) as f32);
        }

        values
    }

    /// Calculate SMA for NEW candles being appended (scrolling right)
    /// Only calculates values starting from `old_len` index
    pub fn calculate_sma_append(&mut self, candles: &[Candle], old_len: usize) {
        let period = self.period;

        // Ensure values vec is sized correctly
        if self.values.len() < old_len {
            self.values.resize(old_len, None);
        }

        // Calculate MA for new candles starting at old_len
        for i in old_len..candles.len() {
            if i < period - 1 {
                // Not enough data for MA yet
                self.values.push(None);
                continue;
            }

            // Use sliding window if we have a previous MA value
            if i >= period && i > 0 && self.values.len() > i - 1 {
                if let Some(prev_ma) = self.values[i - 1] {
                    // Reconstruct sum from previous MA
                    let mut sum = prev_ma as f64 * period as f64;

                    // Sliding window: remove oldest, add newest
                    sum -= candles[i - period].close;
                    sum += candles[i].close;

                    self.values.push(Some((sum / period as f64) as f32));
                    continue;
                }
            }

            // Fallback: calculate from scratch for this value
            // (happens at boundary between old and new data)
            if i >= period - 1 {
                let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                    .iter()
                    .map(|c| c.close)
                    .sum();
                self.values.push(Some((sum / period as f64) as f32));
            } else {
                self.values.push(None);
            }
        }
    }

    /// Calculate SMA for NEW candles being prepended (scrolling left into history)
    /// Calculates values for first `new_count` candles, then prepends to existing values
    pub fn calculate_sma_prepend(&mut self, candles: &[Candle], new_count: usize) {
        let period = self.period;
        let mut new_values = Vec::with_capacity(candles.len());

        // Calculate MA for new candles at the beginning
        for i in 0..new_count {
            if i < period - 1 {
                new_values.push(None);
                continue;
            }

            // Calculate from scratch (can't use sliding window going backwards)
            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();
            new_values.push(Some((sum / period as f64) as f32));
        }

        // Recalculate boundary values that now include prepended candles
        // Need to recalculate up to (period - 1) values after the prepended section
        let boundary_end = (new_count + period - 1).min(candles.len());
        for i in new_count..boundary_end {
            if i < period - 1 {
                new_values.push(None);
                continue;
            }

            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();
            new_values.push(Some((sum / period as f64) as f32));
        }

        // Append remaining old values that don't need recalculation
        if boundary_end < candles.len() {
            let remaining_old_values_start = boundary_end - new_count;
            new_values.extend_from_slice(&self.values[remaining_old_values_start..]);
        }

        self.values = new_values;
    }

    /// Recalculate a specific range of MA values (for boundary corrections)
    pub fn recalculate_range(&mut self, candles: &[Candle], start: usize, end: usize) {
        let period = self.period;
        let end = end.min(candles.len());

        for i in start..end {
            if i < period - 1 {
                if i < self.values.len() {
                    self.values[i] = None;
                }
                continue;
            }

            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();

            let value = Some((sum / period as f64) as f32);

            if i < self.values.len() {
                self.values[i] = value;
            } else {
                self.values.push(value);
            }
        }
    }

    /// Calculate Exponential Moving Average
    pub fn calculate_ema(candles: &[Candle], period: usize) -> Vec<Option<f32>> {
        let mut values = vec![None; candles.len()];

        if candles.len() < period {
            return values;
        }

        // First value is SMA
        let sma: f64 = candles[0..period].iter().map(|c| c.close).sum::<f64>() / period as f64;
        values[period - 1] = Some(sma as f32);

        // Subsequent values use exponential smoothing
        let multiplier = 2.0 / (period as f64 + 1.0);
        for i in period..candles.len() {
            let prev = values[i - 1].unwrap_or(sma as f32);
            let ema = (candles[i].close as f32 - prev) * multiplier as f32 + prev;
            values[i] = Some(ema);
        }

        values
    }

    /// Create a new SMA indicator
    pub fn new_sma(candles: &[Candle], period: usize, color: Color) -> Self {
        Self {
            period,
            values: Self::calculate_sma(candles, period),
            name: format!("SMA-{}", period),
            color,
            visible: true,
        }
    }

    /// Create a new EMA indicator
    pub fn new_ema(candles: &[Candle], period: usize, color: Color) -> Self {
        Self {
            period,
            values: Self::calculate_ema(candles, period),
            name: format!("EMA-{}", period),
            color,
            visible: true,
        }
    }
}

/// Main chart resource
#[derive(Resource)]
pub struct Chart {
    pub ticker_id: i32,
    pub timeframe: String, // "15m", "1h"

    // Data
    pub candles: Vec<Candle>, // Loaded candles (grows as you scroll)
    pub candle_offset: usize, // Global offset (for lazy loading)

    // SHARED X-axis state (synchronized across all panes)
    pub visible_candle_start: usize,
    pub visible_candle_count: usize,

    // Multi-pane support
    pub panes: Vec<Pane>,
    pub total_area: Rect, // Total chart viewport for resize calculations

    // Coordinate system (DEPRECATED - use panes instead)
    pub space: ChartSpace,

    // Indicators
    pub indicators: Vec<MovingAverage>,

    // State
    pub needs_redraw: bool,
    pub loading: bool, // True when fetching more data
}

/// Calculate number of "virtual candles" worth of space to add on the right edge
/// This creates empty space when viewing the latest/newest candles
/// Returns approximately 1/3 of the visible candle count
pub fn right_spacing_candles(visible_candle_count: usize) -> usize {
    visible_candle_count / 3
}

/// Update Y-axis bounds for all panes based on their type
pub fn update_pane_bounds(chart: &mut Chart) {
    // Collect shared data to avoid borrow conflicts
    let candles = &chart.candles;
    let indicators = &chart.indicators;
    let visible_start = chart.visible_candle_start;
    let visible_count = chart.visible_candle_count;

    for pane in chart.panes.iter_mut() {
        match pane.pane_type {
            PaneType::Price => {
                // Use Option B: expand Y-axis to include indicator values
                pane.space.fit_price_bounds_with_indicators(
                    candles,
                    indicators,
                    visible_start,
                    visible_count,
                );
            }
            PaneType::Volume => {
                pane.space
                    .fit_volume_bounds(candles, visible_start, visible_count);
            }
            PaneType::Indicator { .. } => {
                // TODO: Handle indicators when implemented
            }
        }
    }
}

/// Database connection resource (wrapped in Arc<Mutex> for Send + Sync)
#[derive(Resource, Clone)]
pub struct ChartDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl ChartDatabase {
    pub fn new(db_path: &str) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(db_path)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Load candles from database
    pub fn load_candles(
        &self,
        ticker_id: i32,
        timeframe: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<Candle>, duckdb::Error> {
        let query = "
            SELECT candle_time, open_price, high_price, low_price, close_price, volume
            FROM klines
            WHERE ticker_id = ? AND timeframe = ?
                AND candle_time >= ? AND candle_time <= ?
            ORDER BY candle_time ASC
        ";

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(query)?;
        let candles = stmt
            .query_map(params![ticker_id, timeframe, start_time, end_time], |row| {
                Ok(Candle {
                    time: row.get(0)?,
                    open: row.get(1)?,
                    high: row.get(2)?,
                    low: row.get(3)?,
                    close: row.get(4)?,
                    volume: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(candles)
    }

    /// Get total candle count for ticker/timeframe
    pub fn get_candle_count(
        &self,
        ticker_id: i32,
        timeframe: &str,
    ) -> Result<usize, duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM klines WHERE ticker_id = ? AND timeframe = ?")?;
        let count: i64 = stmt.query_row(params![ticker_id, timeframe], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get time range for ticker/timeframe
    pub fn get_time_range(
        &self,
        ticker_id: i32,
        timeframe: &str,
    ) -> Result<(i64, i64), duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT MIN(candle_time), MAX(candle_time) FROM klines WHERE ticker_id = ? AND timeframe = ?"
        )?;
        let (min_time, max_time): (i64, i64) = stmt
            .query_row(params![ticker_id, timeframe], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        Ok((min_time, max_time))
    }

    /// Get available timeframes for a ticker
    pub fn get_available_timeframes(&self, ticker_id: i32) -> Result<Vec<String>, duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT timeframe FROM klines WHERE ticker_id = ? ORDER BY timeframe",
        )?;
        let timeframes = stmt
            .query_map(params![ticker_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        // Sort by timeframe duration order (not alphabetically)
        let mut sorted = timeframes;
        sorted.sort_by_key(|tf| match tf.as_str() {
            "1m" => 1,
            "3m" => 2,
            "5m" => 3,
            "15m" => 4,
            "30m" => 5,
            "1h" => 6,
            "2h" => 7,
            "4h" => 8,
            "6h" => 9,
            "8h" => 10,
            "12h" => 11,
            "1d" => 12,
            "3d" => 13,
            "1w" => 14,
            "1M" => 15,
            _ => 99,
        });

        Ok(sorted)
    }

    /// Resolve ticker symbol to ticker_id
    pub fn resolve_ticker(&self, symbol: &str) -> Result<i32, duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT ticker_id FROM tickers WHERE symbol = ? LIMIT 1")?;
        let ticker_id: i32 = stmt.query_row(params![symbol], |row| row.get(0))?;
        Ok(ticker_id)
    }

    /// Check if data exists for given parameters (returns count)
    pub fn check_data_exists(
        &self,
        ticker_id: i32,
        timeframe: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<usize, duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM klines WHERE ticker_id = ? AND timeframe = ? AND candle_time BETWEEN ? AND ?"
        )?;
        let count: i64 = stmt
            .query_row(params![ticker_id, timeframe, start_time, end_time], |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }
}

/// Interaction state
#[derive(Resource, Default)]
pub struct InteractionState {
    pub mouse_pos: Vec2,
    pub dragging: bool,
    pub drag_start_pos: Vec2,

    // Pane resize state
    pub hover_resize_gap: Option<usize>, // Which gap is being hovered
    pub resizing_gap: Option<usize>,     // Which gap is being dragged
    pub resize_start_heights: Vec<f32>,  // Original height_percent values

    // Crosshair optimization: track last candle to debounce text updates
    pub last_crosshair_candle_index: Option<usize>,
    // Track last mouse position to avoid redundant crosshair updates
    pub last_crosshair_mouse_pos: Vec2,
    // Cache crosshair visibility state to avoid redundant ECS updates
    pub crosshair_visible: bool,
}

/// Volume pane toggle state
#[derive(Resource)]
pub struct VolumeToggleState {
    pub visible: bool,
}

impl Default for VolumeToggleState {
    fn default() -> Self {
        Self { visible: true } // Volume pane visible by default
    }
}

/// Grid configuration
#[derive(Resource)]
pub struct ChartGrid {
    pub show_grid: bool,
    pub grid_color: Color,
    pub y_tick_count: usize, // Number of horizontal grid lines
    pub x_tick_count: usize, // Number of vertical grid lines
}

impl Default for ChartGrid {
    fn default() -> Self {
        Self {
            show_grid: true,
            grid_color: Color::srgba(0.3, 0.3, 0.3, 0.3),
            y_tick_count: 8,
            x_tick_count: 10,
        }
    }
}

/// Axes configuration
#[derive(Resource)]
pub struct ChartAxes {
    pub show_x_labels: bool,
    pub show_y_labels: bool,
    pub label_color: Color,
    pub label_size: f32,
}

impl Default for ChartAxes {
    fn default() -> Self {
        Self {
            show_x_labels: true,
            show_y_labels: true,
            label_color: Color::srgb(0.8, 0.8, 0.8),
            label_size: 16.0,
        }
    }
}

/// Crosshair configuration
#[derive(Resource)]
pub struct Crosshair {
    pub enabled: bool,
    pub line_color: Color,
    pub show_ohlcv_box: bool,
    pub show_price_label: bool,
    pub show_time_label: bool,
}

impl Default for Crosshair {
    fn default() -> Self {
        Self {
            enabled: true,
            line_color: Color::srgba(1.0, 1.0, 1.0, 0.5),
            show_ohlcv_box: true,
            show_price_label: true,
            show_time_label: true,
        }
    }
}

/// Persistent crosshair entities (created once, updated every frame)
#[derive(Resource)]
pub struct CrosshairEntities {
    pub vertical_line_segments: Vec<Entity>, // Multiple dashed segments
    pub horizontal_lines: Vec<(PaneId, Vec<Entity>)>, // Dashed segments per pane
    pub price_labels: Vec<(PaneId, Entity)>, // One label per pane
    pub time_label: Entity,
    pub ohlcv_box: Entity,
}

// ============================================================================
// COMPONENTS
// ============================================================================

/// Component to identify candlestick parts
#[derive(Component)]
pub struct CandlestickWick {
    pub candle_index: usize,
}

#[derive(Component)]
pub struct CandlestickBody {
    pub candle_index: usize,
}

/// Component to identify volume bars
#[derive(Component)]
pub struct VolumeBar {
    pub candle_index: usize,
}

/// Marker component for chart elements
#[derive(Component)]
pub struct ChartElement;

/// Marker component for price pane elements (candlestick wicks and bodies)
#[derive(Component)]
pub struct PriceElement;

/// Marker component for volume pane elements (volume bars)
#[derive(Component)]
pub struct VolumeElement;

/// Marker component for indicator elements (MA lines, etc.)
#[derive(Component)]
pub struct IndicatorElement;

/// Marker component for grid elements (lines and labels)
#[derive(Component)]
pub struct GridElement;

/// Marker component for crosshair elements
#[derive(Component)]
pub struct CrosshairElement;

// ============================================================================
// LOD (LEVEL OF DETAIL) SYSTEM
// ============================================================================

/// Level of detail for candlestick rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleLODLevel {
    /// Full detail: separate wick and body sprites (2 entities per candle)
    Full,
    /// Medium detail: single OHLC line (1 entity per candle)
    Medium,
    /// Low detail: single range line (1 entity per candle)
    Low,
}

/// Marker component for OHLC line entities (Medium LOD)
#[derive(Component)]
pub struct CandlestickOHLCLine {
    pub candle_index: usize,
}

/// Marker component for range line entities (Low LOD)
#[derive(Component)]
pub struct CandlestickRangeLine {
    pub candle_index: usize,
}

/// Configuration for LOD system
#[derive(Resource, Clone)]
pub struct CandlestickLODConfig {
    /// Minimum candle width (px) for full detail rendering
    pub full_detail_threshold: f32,
    /// Minimum candle width (px) for medium detail rendering
    pub medium_detail_threshold: f32,
    /// Minimum candle width (px) to render volume bars
    pub volume_render_threshold: f32,
}

impl Default for CandlestickLODConfig {
    fn default() -> Self {
        Self {
            full_detail_threshold: 3.0,
            medium_detail_threshold: 1.0,
            volume_render_threshold: 0.5,
        }
    }
}

/// Calculate LOD level based on candle width in pixels
pub fn calculate_lod_level(candle_width_px: f32, config: &CandlestickLODConfig) -> CandleLODLevel {
    if candle_width_px >= config.full_detail_threshold {
        CandleLODLevel::Full
    } else if candle_width_px >= config.medium_detail_threshold {
        CandleLODLevel::Medium
    } else {
        CandleLODLevel::Low
    }
}

// ============================================================================
// ENTITY POOLING SYSTEM
// ============================================================================

/// Marker component for pooled entities
#[derive(Component)]
pub struct PooledEntity {
    pub entity_type: PooledEntityType,
    pub in_use: bool,
}

/// Types of pooled entities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PooledEntityType {
    CandlestickWick,
    CandlestickBody,
    CandlestickOHLC,
    CandlestickRange,
    VolumeBar,
}

/// Entity pool for a specific entity type
pub struct EntityPool {
    pub entity_type: PooledEntityType,
    pub all_entities: std::collections::HashSet<Entity>,
    pub available: std::collections::HashSet<Entity>,
}

impl EntityPool {
    pub fn new(entity_type: PooledEntityType) -> Self {
        Self {
            entity_type,
            all_entities: std::collections::HashSet::new(),
            available: std::collections::HashSet::new(),
        }
    }

    /// Get an available entity from the pool (O(1))
    pub fn get(&mut self) -> Option<Entity> {
        self.available.iter().next().copied().map(|entity| {
            self.available.remove(&entity);
            entity
        })
    }

    /// Return an entity to the pool with type validation (O(1))
    pub fn return_entity(&mut self, entity: Entity, pooled_type: PooledEntityType) {
        // Validate entity type
        if pooled_type != self.entity_type {
            eprintln!(
                "WARNING: Attempted to return {:?} entity to {:?} pool",
                pooled_type, self.entity_type
            );
            return;
        }

        if self.all_entities.contains(&entity) {
            self.available.insert(entity);
        }
    }

    /// Add a new entity to the pool
    pub fn add_entity(&mut self, entity: Entity) {
        self.all_entities.insert(entity);
        self.available.insert(entity);
    }

    /// Get pool utilization stats
    pub fn stats(&self) -> (usize, usize) {
        (self.all_entities.len(), self.available.len())
    }
}

/// Resource managing all entity pools
#[derive(Resource)]
pub struct EntityPools {
    pub wicks: EntityPool,
    pub bodies: EntityPool,
    pub ohlc_lines: EntityPool,
    pub range_lines: EntityPool,
    pub volume_bars: EntityPool,
}

impl EntityPools {
    pub fn new() -> Self {
        Self {
            wicks: EntityPool::new(PooledEntityType::CandlestickWick),
            bodies: EntityPool::new(PooledEntityType::CandlestickBody),
            ohlc_lines: EntityPool::new(PooledEntityType::CandlestickOHLC),
            range_lines: EntityPool::new(PooledEntityType::CandlestickRange),
            volume_bars: EntityPool::new(PooledEntityType::VolumeBar),
        }
    }
}

/// Configuration for entity pooling
#[derive(Resource, Clone)]
pub struct EntityPoolConfig {
    pub initial_pool_size: usize,
    pub max_pool_size: usize,
    pub enabled: bool,
}

impl Default for EntityPoolConfig {
    fn default() -> Self {
        Self {
            initial_pool_size: 1000,
            max_pool_size: 2000, // Allow growth but cap it
            enabled: true,
        }
    }
}

// ============================================================================
// TIMEFRAME MANAGEMENT
// ============================================================================

/// Manages available timeframes and current selection
#[derive(Resource, Clone)]
pub struct TimeframeManager {
    pub ticker_id: i32,
    pub current_timeframe: String,
    pub available_timeframes: Vec<String>,
}

impl TimeframeManager {
    pub fn new(ticker_id: i32, current_timeframe: String) -> Self {
        Self {
            ticker_id,
            current_timeframe,
            available_timeframes: Vec::new(),
        }
    }

    /// Get all valid Binance timeframes in order
    pub fn all_timeframes() -> Vec<&'static str> {
        vec![
            "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "6h", "8h", "12h", "1d", "3d", "1w",
            "1M",
        ]
    }

    /// Get the next timeframe in the sequence (only from available timeframes)
    pub fn next_timeframe(&self) -> Option<String> {
        if self.available_timeframes.is_empty() {
            return None;
        }

        // Find current position in available timeframes
        self.available_timeframes
            .iter()
            .position(|tf| tf == &self.current_timeframe)
            .and_then(|idx| self.available_timeframes.get(idx + 1))
            .cloned()
    }

    /// Get the previous timeframe in the sequence (only from available timeframes)
    pub fn prev_timeframe(&self) -> Option<String> {
        if self.available_timeframes.is_empty() {
            return None;
        }

        // Find current position in available timeframes
        self.available_timeframes
            .iter()
            .position(|tf| tf == &self.current_timeframe)
            .and_then(|idx| {
                if idx > 0 {
                    self.available_timeframes.get(idx - 1)
                } else {
                    None
                }
            })
            .cloned()
    }
}

/// Event fired when user requests timeframe change
#[derive(Message, Clone)]
pub struct TimeframeChangeRequest {
    pub from: String,
    pub to: String,
    pub ticker_id: i32,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles(count: usize) -> Vec<Candle> {
        (0..count)
            .map(|i| Candle {
                time: i as i64 * 1000,
                open: (i as f64) * 10.0,
                high: (i as f64) * 10.0 + 5.0,
                low: (i as f64) * 10.0 - 5.0,
                close: (i as f64) * 10.0 + 1.0,
                volume: 1000.0,
            })
            .collect()
    }

    #[test]
    fn test_sma_sliding_window_matches_naive() {
        let candles = create_test_candles(1000);

        let sma_new = MovingAverage::calculate_sma(&candles, 20);
        let sma_old = MovingAverage::calculate_sma_naive(&candles, 20);

        assert_eq!(sma_new.len(), sma_old.len());

        for i in 0..candles.len() {
            match (sma_new[i], sma_old[i]) {
                (Some(a), Some(b)) => {
                    let diff = (a - b).abs();
                    assert!(
                        diff < 0.001,
                        "Mismatch at index {}: new={}, old={}, diff={}",
                        i,
                        a,
                        b,
                        diff
                    );
                }
                (None, None) => {}
                _ => panic!(
                    "Option mismatch at index {}: new={:?}, old={:?}",
                    i, sma_new[i], sma_old[i]
                ),
            }
        }
    }

    #[test]
    fn test_sma_different_periods() {
        let candles = create_test_candles(500);

        for period in [5, 10, 20, 50, 100, 200] {
            let sma_new = MovingAverage::calculate_sma(&candles, period);
            let sma_old = MovingAverage::calculate_sma_naive(&candles, period);

            for i in 0..candles.len() {
                match (sma_new[i], sma_old[i]) {
                    (Some(a), Some(b)) => {
                        assert!(
                            (a - b).abs() < 0.001,
                            "Period {} mismatch at index {}: {} vs {}",
                            period,
                            i,
                            a,
                            b
                        );
                    }
                    (None, None) => {}
                    _ => panic!("Period {} option mismatch at index {}", period, i),
                }
            }
        }
    }

    #[test]
    fn test_sma_edge_case_empty_candles() {
        let candles: Vec<Candle> = vec![];
        let result = MovingAverage::calculate_sma(&candles, 20);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_sma_edge_case_insufficient_candles() {
        let candles = create_test_candles(10);
        let result = MovingAverage::calculate_sma(&candles, 20);
        assert_eq!(result.len(), 10);
        assert!(result.iter().all(|v| v.is_none()));
    }

    #[test]
    fn test_sma_edge_case_exact_period() {
        let candles = create_test_candles(20);
        let result = MovingAverage::calculate_sma(&candles, 20);

        // First 19 should be None
        for i in 0..19 {
            assert!(result[i].is_none(), "Index {} should be None", i);
        }

        // Index 19 should have a value
        assert!(result[19].is_some(), "Index 19 should have a value");
    }

    #[test]
    fn test_sma_edge_case_period_one() {
        let candles = create_test_candles(10);
        let result = MovingAverage::calculate_sma(&candles, 1);

        // All values should equal close prices
        for i in 0..candles.len() {
            match result[i] {
                Some(v) => {
                    assert_eq!(v, candles[i].close as f32);
                }
                None => panic!("Expected value at index {}", i),
            }
        }
    }

    #[test]
    fn test_sma_numerical_stability() {
        // Test with large numbers to check floating point stability
        let candles: Vec<Candle> = (0..100)
            .map(|i| Candle {
                time: i,
                open: 1_000_000.0 + i as f64,
                high: 1_000_000.0 + i as f64,
                low: 1_000_000.0 + i as f64,
                close: 1_000_000.0 + i as f64,
                volume: 1000.0,
            })
            .collect();

        let sma_new = MovingAverage::calculate_sma(&candles, 20);
        let sma_old = MovingAverage::calculate_sma_naive(&candles, 20);

        for i in 20..candles.len() {
            if let (Some(a), Some(b)) = (sma_new[i], sma_old[i]) {
                let relative_error = ((a - b) / b).abs();
                assert!(
                    relative_error < 0.00001,
                    "Numerical instability at {}: {} vs {}, error: {}",
                    i,
                    a,
                    b,
                    relative_error
                );
            }
        }
    }

    #[test]
    fn test_sma_append_incremental() {
        // Create initial dataset
        let mut candles = create_test_candles(1000);

        // Calculate full SMA
        let mut ma_incremental = MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 1.0, 1.0));
        let full_reference = ma_incremental.values.clone();

        // Add more candles
        let old_len = candles.len();
        candles.extend(create_test_candles(100).iter().map(|c| Candle {
            time: c.time + old_len as i64 * 1000,
            open: c.open + old_len as f64 * 10.0,
            high: c.high + old_len as f64 * 10.0,
            low: c.low + old_len as f64 * 10.0,
            close: c.close + old_len as f64 * 10.0,
            volume: c.volume,
        }));

        // Calculate incrementally
        ma_incremental.calculate_sma_append(&candles, old_len);

        // Calculate from scratch for comparison
        let ma_full = MovingAverage::calculate_sma(&candles, 20);

        // Verify old values unchanged
        for i in 0..old_len {
            match (ma_incremental.values[i], full_reference[i]) {
                (Some(a), Some(b)) => {
                    assert_eq!(a, b, "Old value changed at index {}", i);
                }
                (None, None) => {}
                _ => panic!("Old value changed at index {}", i),
            }
        }

        // Verify new values match full calculation
        for i in old_len..candles.len() {
            match (ma_incremental.values[i], ma_full[i]) {
                (Some(a), Some(b)) => {
                    assert!(
                        (a - b).abs() < 0.001,
                        "New value mismatch at index {}: {} vs {}",
                        i,
                        a,
                        b
                    );
                }
                (None, None) => {}
                _ => panic!("New value mismatch at index {}", i),
            }
        }
    }

    #[test]
    fn test_sma_prepend_incremental() {
        // Create initial dataset starting at index 100
        let candles_old = create_test_candles(100)
            .iter()
            .map(|c| Candle {
                time: c.time + 100_000,
                open: c.open + 1000.0,
                high: c.high + 1000.0,
                low: c.low + 1000.0,
                close: c.close + 1000.0,
                volume: c.volume,
            })
            .collect::<Vec<_>>();

        // Calculate SMA for old data
        let mut ma_incremental =
            MovingAverage::new_sma(&candles_old, 20, Color::srgb(1.0, 1.0, 1.0));

        // Create full dataset (new + old)
        let mut candles_full = create_test_candles(100);
        candles_full.extend(candles_old);

        // Calculate incrementally (prepend)
        ma_incremental.calculate_sma_prepend(&candles_full, 100);

        // Calculate from scratch for comparison
        let ma_full = MovingAverage::calculate_sma(&candles_full, 20);

        // Verify entire result matches
        assert_eq!(ma_incremental.values.len(), ma_full.len());

        for i in 0..candles_full.len() {
            match (ma_incremental.values[i], ma_full[i]) {
                (Some(a), Some(b)) => {
                    assert!(
                        (a - b).abs() < 0.001,
                        "Mismatch at index {}: {} vs {}",
                        i,
                        a,
                        b
                    );
                }
                (None, None) => {}
                _ => panic!("Option mismatch at index {}", i),
            }
        }
    }

    #[test]
    fn test_sma_append_boundary_conditions() {
        let mut candles = create_test_candles(50);
        let mut ma = MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 1.0, 1.0));

        // Append just 1 candle
        let old_len = candles.len();
        candles.push(Candle {
            time: 50000,
            open: 500.0,
            high: 505.0,
            low: 495.0,
            close: 501.0,
            volume: 1000.0,
        });

        ma.calculate_sma_append(&candles, old_len);

        // Should have exactly one new value
        assert_eq!(ma.values.len(), 51);
        assert!(ma.values[50].is_some());
    }

    #[test]
    fn test_sma_prepend_insufficient_period() {
        // Create candles where new data doesn't have enough for period
        let candles_old = create_test_candles(100);
        let mut ma = MovingAverage::new_sma(&candles_old, 50, Color::srgb(1.0, 1.0, 1.0));

        // Prepend only 10 candles (not enough for period=50)
        let mut candles_full = create_test_candles(10);
        candles_full.extend(candles_old);

        ma.calculate_sma_prepend(&candles_full, 10);

        // First 10 values should be None (not enough for period)
        for i in 0..10 {
            assert!(ma.values[i].is_none(), "Index {} should be None", i);
        }
    }

    #[test]
    fn test_sma_append_different_periods() {
        // Test incremental append with various periods
        for period in [20, 50, 100] {
            let mut candles = create_test_candles(500);
            let mut ma = MovingAverage::new_sma(&candles, period, Color::srgb(1.0, 1.0, 1.0));

            let old_len = candles.len();
            candles.extend(create_test_candles(100));

            ma.calculate_sma_append(&candles, old_len);
            let ma_full = MovingAverage::calculate_sma(&candles, period);

            for i in old_len..candles.len() {
                match (ma.values[i], ma_full[i]) {
                    (Some(a), Some(b)) => {
                        assert!(
                            (a - b).abs() < 0.001,
                            "Period {} mismatch at {}: {} vs {}",
                            period,
                            i,
                            a,
                            b
                        );
                    }
                    (None, None) => {}
                    _ => panic!("Period {} option mismatch at {}", period, i),
                }
            }
        }
    }
}
