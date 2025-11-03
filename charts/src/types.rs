use bevy::prelude::*;
use duckdb::{params, Connection};
use std::sync::{Arc, Mutex};

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// Raw candle from database
#[derive(Debug, Clone)]
pub struct Candle {
    pub time: i64,           // Unix timestamp in ms
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
    pub visible_price_min: f32,        // Auto-calculated from visible candles
    pub visible_price_max: f32,        // Auto-calculated from visible candles
    pub price_padding: f32,            // Extra space above/below (5-10%)

    // Physical viewport (Bevy world space, pane-specific)
    pub viewport: Rect,                // Where this pane is drawn on screen

    // Cached calculations (updated when bounds change)
    pub candle_width_px: f32,          // Width of each candle in pixels
    pub price_scale: f32,              // Pixels per $1 price movement
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
    pub fn to_world(&self, candle_index: usize, price: f32, visible_candle_start: usize, visible_candle_count: usize) -> Vec2 {
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
    pub fn from_world(&self, world_pos: Vec2, visible_candle_start: usize, visible_candle_count: usize) -> (usize, f32) {
        let x_percent = (world_pos.x - self.viewport.min.x) / self.viewport.width();
        let candle_index = visible_candle_start +
            (x_percent * visible_candle_count as f32) as usize;

        let y_percent = (world_pos.y - self.viewport.min.y) / self.viewport.height();
        let price = self.visible_price_min +
            y_percent * (self.visible_price_max - self.visible_price_min);

        (candle_index, price)
    }

    /// Fit price bounds to visible candles
    pub fn fit_price_bounds(&mut self, candles: &[Candle], visible_candle_start: usize, visible_candle_count: usize) {
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

    /// Fit volume bounds to visible candles
    pub fn fit_volume_bounds(&mut self, candles: &[Candle], visible_candle_start: usize, visible_candle_count: usize) {
        if candles.is_empty() {
            return;
        }

        let start = visible_candle_start;
        let end = (start + visible_candle_count).min(candles.len());

        if start >= end {
            return;
        }

        let visible_candles = &candles[start..end];
        let max_volume = visible_candles.iter()
            .map(|c| c.volume)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(1.0);

        self.visible_price_min = 0.0;
        self.visible_price_max = max_volume as f32;

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
    pub fn new(id: PaneId, pane_type: PaneType, height_percent: f32, viewport: Rect, visible_candle_count: usize) -> Self {
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

    const SEPARATOR_GAP: f32 = 40.0; // Padding between panes

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

/// Main chart resource
#[derive(Resource)]
pub struct Chart {
    pub ticker_id: i32,
    pub timeframe: String,             // "15m", "1h"

    // Data
    pub candles: Vec<Candle>,          // Loaded candles (grows as you scroll)
    pub candle_offset: usize,          // Global offset (for lazy loading)

    // SHARED X-axis state (synchronized across all panes)
    pub visible_candle_start: usize,
    pub visible_candle_count: usize,

    // Multi-pane support
    pub panes: Vec<Pane>,
    pub total_area: Rect,              // Total chart viewport for resize calculations

    // Coordinate system (DEPRECATED - use panes instead)
    pub space: ChartSpace,

    // State
    pub needs_redraw: bool,
    pub loading: bool,                 // True when fetching more data
}

/// Update Y-axis bounds for all panes based on their type
pub fn update_pane_bounds(chart: &mut Chart) {
    for pane in chart.panes.iter_mut() {
        match pane.pane_type {
            PaneType::Price => {
                pane.space.fit_price_bounds(
                    &chart.candles,
                    chart.visible_candle_start,
                    chart.visible_candle_count
                );
            }
            PaneType::Volume => {
                pane.space.fit_volume_bounds(
                    &chart.candles,
                    chart.visible_candle_start,
                    chart.visible_candle_count
                );
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
        let candles = stmt.query_map(
            params![ticker_id, timeframe, start_time, end_time],
            |row| {
                Ok(Candle {
                    time: row.get(0)?,
                    open: row.get(1)?,
                    high: row.get(2)?,
                    low: row.get(3)?,
                    close: row.get(4)?,
                    volume: row.get(5)?,
                })
            }
        )?.collect::<Result<Vec<_>, _>>()?;

        Ok(candles)
    }

    /// Get total candle count for ticker/timeframe
    pub fn get_candle_count(&self, ticker_id: i32, timeframe: &str)
        -> Result<usize, duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM klines WHERE ticker_id = ? AND timeframe = ?"
        )?;
        let count: i64 = stmt.query_row(
            params![ticker_id, timeframe],
            |row| row.get(0)
        )?;
        Ok(count as usize)
    }

    /// Get time range for ticker/timeframe
    pub fn get_time_range(&self, ticker_id: i32, timeframe: &str)
        -> Result<(i64, i64), duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT MIN(candle_time), MAX(candle_time) FROM klines WHERE ticker_id = ? AND timeframe = ?"
        )?;
        let (min_time, max_time): (i64, i64) = stmt.query_row(
            params![ticker_id, timeframe],
            |row| Ok((row.get(0)?, row.get(1)?))
        )?;
        Ok((min_time, max_time))
    }
}

/// Interaction state
#[derive(Resource, Default)]
pub struct InteractionState {
    pub mouse_pos: Vec2,
    pub dragging: bool,
    pub drag_start_pos: Vec2,

    // Pane resize state
    pub hover_resize_gap: Option<usize>,    // Which gap is being hovered
    pub resizing_gap: Option<usize>,        // Which gap is being dragged
    pub resize_start_heights: Vec<f32>,     // Original height_percent values

    // Crosshair optimization: track last candle to debounce text updates
    pub last_crosshair_candle_index: Option<usize>,
}

/// Grid configuration
#[derive(Resource)]
pub struct ChartGrid {
    pub show_grid: bool,
    pub grid_color: Color,
    pub y_tick_count: usize,      // Number of horizontal grid lines
    pub x_tick_count: usize,      // Number of vertical grid lines
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
    pub vertical_line: Entity,
    pub horizontal_lines: Vec<(PaneId, Entity)>,  // One line per pane
    pub price_labels: Vec<(PaneId, Entity)>,      // One label per pane
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

/// Marker component for grid elements (lines and labels)
#[derive(Component)]
pub struct GridElement;

/// Marker component for crosshair elements
#[derive(Component)]
pub struct CrosshairElement;
