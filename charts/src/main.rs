use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use duckdb::{params, Connection};
use std::sync::{Arc, Mutex};

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// Raw candle from database
#[derive(Debug, Clone)]
struct Candle {
    time: i64,           // Unix timestamp in ms
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

/// Coordinate space for chart rendering
#[derive(Debug, Clone)]
struct ChartSpace {
    // Y-axis: Price/Value axis (logical, pane-specific)
    visible_price_min: f32,        // Auto-calculated from visible candles
    visible_price_max: f32,        // Auto-calculated from visible candles
    price_padding: f32,            // Extra space above/below (5-10%)

    // Physical viewport (Bevy world space, pane-specific)
    viewport: Rect,                // Where this pane is drawn on screen

    // Cached calculations (updated when bounds change)
    candle_width_px: f32,          // Width of each candle in pixels
    price_scale: f32,              // Pixels per $1 price movement
}

impl ChartSpace {
    fn new(viewport: Rect, visible_candle_count: usize) -> Self {
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
    fn to_world(&self, candle_index: usize, price: f32, visible_candle_start: usize, visible_candle_count: usize) -> Vec2 {
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
    fn from_world(&self, world_pos: Vec2, visible_candle_start: usize, visible_candle_count: usize) -> (usize, f32) {
        let x_percent = (world_pos.x - self.viewport.min.x) / self.viewport.width();
        let candle_index = visible_candle_start +
            (x_percent * visible_candle_count as f32) as usize;

        let y_percent = (world_pos.y - self.viewport.min.y) / self.viewport.height();
        let price = self.visible_price_min +
            y_percent * (self.visible_price_max - self.visible_price_min);

        (candle_index, price)
    }

    /// Fit price bounds to visible candles
    fn fit_price_bounds(&mut self, candles: &[Candle], visible_candle_start: usize, visible_candle_count: usize) {
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
    fn fit_volume_bounds(&mut self, candles: &[Candle], visible_candle_start: usize, visible_candle_count: usize) {
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
    fn recalculate_cache(&mut self, visible_candle_count: usize) {
        self.candle_width_px = self.viewport.width() / visible_candle_count as f32;
        let price_range = self.visible_price_max - self.visible_price_min;
        self.price_scale = if price_range > 0.0 {
            self.viewport.height() / price_range
        } else {
            1.0
        };
    }
}

/// Pane identifier
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum PaneId {
    Price,
    Volume,
    Indicator(usize),
}

/// What type of content this pane renders
#[derive(Clone, Debug)]
enum PaneType {
    Price,
    Volume,
    Indicator { name: String },
}

/// Individual pane configuration
#[derive(Clone)]
struct Pane {
    id: PaneId,
    pane_type: PaneType,
    height_percent: f32,
    space: ChartSpace,
}

impl Pane {
    fn new(id: PaneId, pane_type: PaneType, height_percent: f32, viewport: Rect, visible_candle_count: usize) -> Self {
        Self {
            id,
            pane_type,
            height_percent,
            space: ChartSpace::new(viewport, visible_candle_count),
        }
    }
}

/// Calculate and assign viewports to each pane based on height percentages
/// Panes are stacked from top to bottom
fn calculate_pane_layouts(panes: &mut [Pane], total_area: Rect, visible_candle_count: usize) {
    if panes.is_empty() {
        return;
    }

    // Start from the top
    let mut current_y = total_area.max.y;
    let total_height = total_area.height();
    let total_width = total_area.width();

    for pane in panes.iter_mut() {
        let pane_height = total_height * pane.height_percent;
        let pane_min_y = current_y - pane_height;
        let pane_max_y = current_y;

        // Create viewport for this pane
        pane.space.viewport = Rect::from_corners(
            Vec2::new(total_area.min.x, pane_min_y),
            Vec2::new(total_area.max.x, pane_max_y),
        );

        // Recalculate cached values
        pane.space.recalculate_cache(visible_candle_count);

        // Move down for next pane
        current_y = pane_min_y;
    }
}

/// Main chart resource
#[derive(Resource)]
struct Chart {
    ticker_id: i32,
    timeframe: String,             // "15m", "1h"

    // Data
    candles: Vec<Candle>,          // Loaded candles (grows as you scroll)
    candle_offset: usize,          // Global offset (for lazy loading)

    // SHARED X-axis state (synchronized across all panes)
    visible_candle_start: usize,
    visible_candle_count: usize,

    // Multi-pane support
    panes: Vec<Pane>,

    // Coordinate system (DEPRECATED - use panes instead)
    space: ChartSpace,

    // State
    needs_redraw: bool,
    loading: bool,                 // True when fetching more data
}

/// Database connection resource (wrapped in Arc<Mutex> for Send + Sync)
#[derive(Resource, Clone)]
struct ChartDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl ChartDatabase {
    /// Load candles from database
    fn load_candles(
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
    fn get_candle_count(&self, ticker_id: i32, timeframe: &str)
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
    fn get_time_range(&self, ticker_id: i32, timeframe: &str)
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
struct InteractionState {
    mouse_pos: Vec2,
    dragging: bool,
    drag_start_pos: Vec2,
}

/// Grid configuration
#[derive(Resource)]
struct ChartGrid {
    show_grid: bool,
    grid_color: Color,
    y_tick_count: usize,      // Number of horizontal grid lines
    x_tick_count: usize,      // Number of vertical grid lines
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
struct ChartAxes {
    show_x_labels: bool,
    show_y_labels: bool,
    label_color: Color,
    label_size: f32,
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
struct Crosshair {
    enabled: bool,
    line_color: Color,
    show_ohlcv_box: bool,
    show_price_label: bool,
    show_time_label: bool,
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

// ============================================================================
// COMPONENTS
// ============================================================================

/// Component to identify candlestick parts
#[derive(Component)]
struct CandlestickWick {
    candle_index: usize,
}

#[derive(Component)]
struct CandlestickBody {
    candle_index: usize,
}

/// Marker component for chart elements
#[derive(Component)]
struct ChartElement;

/// Marker component for grid elements (lines and labels)
#[derive(Component)]
struct GridElement;

/// Marker component for crosshair elements
#[derive(Component)]
struct CrosshairElement;

// ============================================================================
// SYSTEMS
// ============================================================================

fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn(Camera2dBundle::default());

    // Initialize database connection
    let db_path = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb";
    let conn = Connection::open(db_path)
        .expect("Failed to open database");

    let db = ChartDatabase { conn: Arc::new(Mutex::new(conn)) };

    // Load initial data
    let ticker_id = 1; // BTCUSDT
    let timeframe = "15m";

    // Get time range and load most recent candles
    let (min_time, max_time) = db.get_time_range(ticker_id, timeframe)
        .expect("Failed to get time range");

    println!("Database time range: {} to {}", min_time, max_time);

    let candles = db.load_candles(ticker_id, timeframe, min_time, max_time)
        .expect("Failed to load candles");

    println!("Loaded {} candles", candles.len());

    // Total chart area (leave room for axes labels)
    // Window is 1600x900, but we need margins for labels
    let total_area = Rect::from_center_size(
        Vec2::new(-40.0, 10.0),  // Offset left and up slightly
        Vec2::new(1400.0, 780.0), // Smaller than window to leave room for labels
    );

    let visible_candle_count = 50.min(candles.len());
    let visible_candle_start = candles.len().saturating_sub(visible_candle_count);

    // Initialize multi-pane layout: 70% Price + 30% Volume
    let mut panes = vec![
        Pane::new(
            PaneId::Price,
            PaneType::Price,
            0.7, // 70% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            visible_candle_count,
        ),
        Pane::new(
            PaneId::Volume,
            PaneType::Volume,
            0.3, // 30% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            visible_candle_count,
        ),
    ];

    // Calculate pane layouts
    calculate_pane_layouts(&mut panes, total_area, visible_candle_count);

    // Fit Y-axis bounds for each pane
    for pane in panes.iter_mut() {
        match pane.pane_type {
            PaneType::Price => {
                pane.space.fit_price_bounds(&candles, visible_candle_start, visible_candle_count);
            }
            PaneType::Volume => {
                pane.space.fit_volume_bounds(&candles, visible_candle_start, visible_candle_count);
            }
            _ => {}
        }
    }

    // Create deprecated space for backward compatibility (not used in multi-pane)
    let space = ChartSpace::new(total_area, visible_candle_count);

    let chart = Chart {
        ticker_id,
        timeframe: timeframe.to_string(),
        candles,
        candle_offset: 0,
        visible_candle_start,
        visible_candle_count,
        panes,
        space, // Deprecated
        needs_redraw: true,
        loading: false,
    };

    commands.insert_resource(db);
    commands.insert_resource(chart);
    commands.insert_resource(InteractionState::default());
    commands.insert_resource(ChartGrid::default());
    commands.insert_resource(ChartAxes::default());
    commands.insert_resource(Crosshair::default());

    println!("Setup complete!");
}

fn render_candlesticks(
    mut commands: Commands,
    chart: Res<Chart>,
    query: Query<Entity, With<ChartElement>>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn all existing chart elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Find the Price pane
    let price_pane = chart.panes.iter()
        .find(|p| matches!(p.id, PaneId::Price));

    if price_pane.is_none() {
        return;
    }
    let price_pane = price_pane.unwrap();

    // Get shared X-axis state
    let start = chart.visible_candle_start;
    let end = (start + chart.visible_candle_count).min(chart.candles.len());

    if start >= end {
        return;
    }

    for i in start..end {
        let candle = &chart.candles[i];

        // Calculate positions using ChartSpace::to_world() with shared X-axis params
        let wick_bottom = price_pane.space.to_world(
            i, candle.low as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let wick_top = price_pane.space.to_world(
            i, candle.high as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let body_open = price_pane.space.to_world(
            i, candle.open as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let body_close = price_pane.space.to_world(
            i, candle.close as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );

        let wick_center = Vec2::new(
            (wick_bottom.x + wick_top.x) / 2.0,
            (wick_bottom.y + wick_top.y) / 2.0,
        );
        let wick_height = (wick_top.y - wick_bottom.y).abs().max(1.0);

        // Spawn wick entity (thin line, Z=0)
        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgb(0.5, 0.5, 0.5),
                    custom_size: Some(Vec2::new(1.0, wick_height)),
                    ..default()
                },
                transform: Transform::from_translation(wick_center.extend(0.0)),
                ..default()
            },
            CandlestickWick { candle_index: i },
            ChartElement,
            PaneId::Price,
        ));

        // Spawn body entity (rectangle, Z=1 above wick)
        let body_width = price_pane.space.candle_width_px * 0.7;
        let body_height = (body_close.y - body_open.y).abs().max(1.0);
        let body_center = Vec2::new(
            (body_open.x + body_close.x) / 2.0,
            (body_open.y + body_close.y) / 2.0,
        );

        let body_color = if candle.close >= candle.open {
            Color::srgb(0.0, 0.8, 0.2)  // Green
        } else {
            Color::srgb(0.9, 0.2, 0.2)  // Red
        };

        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: body_color,
                    custom_size: Some(Vec2::new(body_width, body_height)),
                    ..default()
                },
                transform: Transform::from_translation(body_center.extend(1.0)),
                ..default()
            },
            CandlestickBody { candle_index: i },
            ChartElement,
            PaneId::Price,
        ));
    }

    println!(
        "Rendered {} candles (indices {}-{})",
        end - start,
        start,
        end - 1
    );
}

/// Component to identify volume bars
#[derive(Component)]
struct VolumeBar {
    candle_index: usize,
}

fn render_volume_bars(
    mut commands: Commands,
    chart: Res<Chart>,
    query: Query<Entity, (With<VolumeBar>, With<ChartElement>)>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn all existing volume bars
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Find the Volume pane
    let volume_pane = chart.panes.iter()
        .find(|p| matches!(p.id, PaneId::Volume));

    if volume_pane.is_none() {
        return;
    }
    let volume_pane = volume_pane.unwrap();

    // Get shared X-axis state
    let start = chart.visible_candle_start;
    let end = (start + chart.visible_candle_count).min(chart.candles.len());

    if start >= end {
        return;
    }

    for i in start..end {
        let candle = &chart.candles[i];

        // Calculate bottom (0) and top (volume) positions
        let bar_bottom = volume_pane.space.to_world(
            i, 0.0,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let bar_top = volume_pane.space.to_world(
            i, candle.volume as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );

        let bar_center = Vec2::new(
            (bar_bottom.x + bar_top.x) / 2.0,
            (bar_bottom.y + bar_top.y) / 2.0,
        );
        let bar_height = (bar_top.y - bar_bottom.y).abs().max(1.0);
        let bar_width = volume_pane.space.candle_width_px * 0.7;

        // Color based on candle direction
        let bar_color = if candle.close >= candle.open {
            Color::srgba(0.0, 0.8, 0.2, 0.6)  // Green with transparency
        } else {
            Color::srgba(0.9, 0.2, 0.2, 0.6)  // Red with transparency
        };

        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: bar_color,
                    custom_size: Some(Vec2::new(bar_width, bar_height)),
                    ..default()
                },
                transform: Transform::from_translation(bar_center.extend(0.0)),
                ..default()
            },
            VolumeBar { candle_index: i },
            ChartElement,
            PaneId::Volume,
        ));
    }

    println!(
        "Rendered {} volume bars (indices {}-{})",
        end - start,
        start,
        end - 1
    );
}

fn render_grid_and_axes(
    mut commands: Commands,
    chart: Res<Chart>,
    grid: Res<ChartGrid>,
    axes: Res<ChartAxes>,
    query: Query<Entity, With<GridElement>>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn existing grid elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if !grid.show_grid || chart.panes.is_empty() {
        return;
    }

    // Calculate total chart bounds (from top of first pane to bottom of last pane)
    let first_pane = &chart.panes[0];
    let last_pane = &chart.panes[chart.panes.len() - 1];
    let chart_top = first_pane.space.viewport.max.y;
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_left = first_pane.space.viewport.min.x;
    let chart_right = first_pane.space.viewport.max.x;

    // ========== PER-PANE HORIZONTAL GRID LINES & Y-AXIS LABELS ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        for i in 0..=grid.y_tick_count {
            let value_percent = i as f32 / grid.y_tick_count as f32;
            let value = pane.space.visible_price_min +
                value_percent * (pane.space.visible_price_max - pane.space.visible_price_min);

            let y = viewport.min.y + value_percent * viewport.height();

            // Draw horizontal line
            let line_center = Vec2::new((chart_left + chart_right) / 2.0, y);
            let line_width = chart_right - chart_left;

            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: grid.grid_color,
                        custom_size: Some(Vec2::new(line_width, 1.0)),
                        ..default()
                    },
                    transform: Transform::from_translation(line_center.extend(-1.0)),
                    ..default()
                },
                GridElement,
            ));

            // Y-axis label on the right side
            if axes.show_y_labels {
                let label_x = chart_right + 50.0;
                let label_text = format!("{:.2}", value);

                commands.spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            label_text,
                            TextStyle {
                                font_size: axes.label_size,
                                color: axes.label_color,
                                ..default()
                            },
                        ),
                        transform: Transform::from_translation(Vec3::new(label_x, y, 2.0)),
                        text_anchor: bevy::sprite::Anchor::CenterLeft,
                        ..default()
                    },
                    GridElement,
                ));
            }
        }
    }

    // ========== PANE SEPARATORS ==========
    for i in 0..chart.panes.len() - 1 {
        let pane_bottom = chart.panes[i].space.viewport.min.y;
        let separator_y = pane_bottom;
        let line_center = Vec2::new((chart_left + chart_right) / 2.0, separator_y);
        let line_width = chart_right - chart_left;

        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgba(0.5, 0.5, 0.5, 0.5), // Semi-transparent gray
                    custom_size: Some(Vec2::new(line_width, 2.0)), // Thicker line
                    ..default()
                },
                transform: Transform::from_translation(line_center.extend(0.5)),
                ..default()
            },
            GridElement,
        ));
    }

    // ========== VERTICAL GRID LINES (Time - spans all panes) ==========
    for i in 0..=grid.x_tick_count {
        let candle_percent = i as f32 / grid.x_tick_count as f32;
        let candle_index = chart.visible_candle_start +
            (candle_percent * chart.visible_candle_count as f32) as usize;

        if candle_index >= chart.candles.len() {
            continue;
        }

        let x = chart_left + candle_percent * (chart_right - chart_left);

        // Draw vertical line spanning all panes
        let line_center = Vec2::new(x, (chart_bottom + chart_top) / 2.0);
        let line_height = chart_top - chart_bottom;

        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: grid.grid_color,
                    custom_size: Some(Vec2::new(1.0, line_height)),
                    ..default()
                },
                transform: Transform::from_translation(line_center.extend(-1.0)),
                ..default()
            },
            GridElement,
        ));

        // X-axis label (time) at the bottom of the last pane
        if axes.show_x_labels {
            let candle = &chart.candles[candle_index];
            let label_y = chart_bottom - 40.0;

            // Format timestamp using chrono
            use chrono::{DateTime, Utc};
            let datetime = DateTime::<Utc>::from_timestamp(candle.time / 1000, 0)
                .unwrap_or_default();
            let label_text = datetime.format("%m/%d %H:%M").to_string();

            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        label_text,
                        TextStyle {
                            font_size: axes.label_size,
                            color: axes.label_color,
                            ..default()
                        },
                    ),
                    transform: Transform::from_translation(Vec3::new(x, label_y, 2.0)),
                    text_anchor: bevy::sprite::Anchor::Center,
                    ..default()
                },
                GridElement,
            ));
        }
    }
}

fn render_crosshair(
    mut commands: Commands,
    chart: Res<Chart>,
    crosshair: Res<Crosshair>,
    interaction: Res<InteractionState>,
    query: Query<Entity, With<CrosshairElement>>,
) {
    // Despawn existing crosshair elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if !crosshair.enabled || chart.panes.is_empty() {
        return;
    }

    // Calculate total chart bounds (from top of first pane to bottom of last pane)
    let first_pane = &chart.panes[0];
    let last_pane = &chart.panes[chart.panes.len() - 1];
    let chart_top = first_pane.space.viewport.max.y;
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_left = first_pane.space.viewport.min.x;
    let chart_right = first_pane.space.viewport.max.x;

    // Check if mouse is within any pane
    let mouse_in_chart = chart.panes.iter().any(|pane| {
        pane.space.viewport.contains(interaction.mouse_pos)
    });

    if !mouse_in_chart {
        return;
    }

    let mouse_x = interaction.mouse_pos.x;
    let mouse_y = interaction.mouse_pos.y;

    // ========== VERTICAL CROSSHAIR LINE (spans all panes) ==========
    let total_height = chart_top - chart_bottom;
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: crosshair.line_color,
                custom_size: Some(Vec2::new(1.0, total_height)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(
                mouse_x,
                (chart_top + chart_bottom) / 2.0,
                3.0,
            )),
            ..default()
        },
        CrosshairElement,
    ));

    // ========== PER-PANE HORIZONTAL CROSSHAIR LINES & VALUE LABELS ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        // Check if mouse Y is within this pane
        if mouse_y >= viewport.min.y && mouse_y <= viewport.max.y {
            // Draw horizontal line within this pane
            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: crosshair.line_color,
                        custom_size: Some(Vec2::new(viewport.width(), 1.0)),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(
                        viewport.center().x,
                        mouse_y,
                        3.0,
                    )),
                    ..default()
                },
                CrosshairElement,
            ));

            // Show value label for this pane
            if crosshair.show_price_label {
                let (_, value_at_cursor) = pane.space.from_world(
                    interaction.mouse_pos,
                    chart.visible_candle_start,
                    chart.visible_candle_count
                );
                let label_text = format!("{:.2}", value_at_cursor);
                let label_x = chart_right + 50.0;

                commands.spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            label_text,
                            TextStyle {
                                font_size: 14.0,
                                color: Color::srgb(1.0, 1.0, 0.0), // Yellow for crosshair
                                ..default()
                            },
                        ),
                        transform: Transform::from_translation(Vec3::new(label_x, mouse_y, 4.0)),
                        text_anchor: bevy::sprite::Anchor::CenterLeft,
                        ..default()
                    },
                    CrosshairElement,
                ));
            }
        }
    }

    // ========== FIND CANDLE AT CURSOR ==========
    let (candle_index, _) = if let Some(pane) = chart.panes.first() {
        pane.space.from_world(interaction.mouse_pos, chart.visible_candle_start, chart.visible_candle_count)
    } else {
        return;
    };

    if candle_index >= chart.candles.len() {
        return;
    }

    let candle = &chart.candles[candle_index];

    // ========== TIME LABEL AT CROSSHAIR ==========
    if crosshair.show_time_label {
        use chrono::{DateTime, Utc};
        let datetime = DateTime::<Utc>::from_timestamp(candle.time / 1000, 0)
            .unwrap_or_default();
        let label_text = datetime.format("%m/%d %H:%M").to_string();
        let label_y = chart_bottom - 40.0;

        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    label_text,
                    TextStyle {
                        font_size: 14.0,
                        color: Color::srgb(1.0, 1.0, 0.0), // Yellow for crosshair time
                        ..default()
                    },
                ),
                transform: Transform::from_translation(Vec3::new(mouse_x, label_y, 4.0)),
                text_anchor: bevy::sprite::Anchor::Center,
                ..default()
            },
            CrosshairElement,
        ));
    }

    // ========== OHLCV INFO BOX (at top of Price pane) ==========
    if crosshair.show_ohlcv_box {
        let info_text = format!(
            "O: {:.2}  H: {:.2}  L: {:.2}  C: {:.2}\nVol: {:.2}",
            candle.open, candle.high, candle.low, candle.close, candle.volume
        );

        // Find Price pane and position box at its top-left
        if let Some(price_pane) = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price)) {
            let info_x = price_pane.space.viewport.min.x + 100.0;
            let info_y = price_pane.space.viewport.max.y - 40.0;

            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        info_text,
                        TextStyle {
                            font_size: 16.0,
                            color: Color::srgb(1.0, 1.0, 1.0),
                            ..default()
                        },
                    ),
                    transform: Transform::from_translation(Vec3::new(info_x, info_y, 4.0)),
                    text_anchor: bevy::sprite::Anchor::TopLeft,
                    ..default()
                },
                CrosshairElement,
            ));
        }
    }
}

fn handle_mouse_input(
    mut chart: ResMut<Chart>,
    mut interaction: ResMut<InteractionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: EventReader<MouseWheel>,
    window_query: Query<&Window>,
) {
    let Ok(window) = window_query.get_single() else {
        return;
    };

    // Update mouse position (convert to world space)
    // Bevy's cursor_position() has (0,0) at top-left with Y down
    // World space has (0,0) at center with Y up - need to flip Y
    if let Some(cursor_pos) = window.cursor_position() {
        let window_size = Vec2::new(window.width(), window.height());
        interaction.mouse_pos = Vec2::new(
            cursor_pos.x - window_size.x / 2.0,
            window_size.y / 2.0 - cursor_pos.y,  // Flip Y axis
        );
    }

    // Check if mouse is in any pane
    let mouse_in_pane = chart.panes.iter()
        .any(|pane| pane.space.viewport.contains(interaction.mouse_pos));

    // Pan: Left mouse button drag
    if mouse_button.just_pressed(MouseButton::Left) {
        if mouse_in_pane {
            interaction.dragging = true;
            interaction.drag_start_pos = interaction.mouse_pos;
        }
    }

    if mouse_button.just_released(MouseButton::Left) {
        interaction.dragging = false;
    }

    if interaction.dragging && !chart.panes.is_empty() {
        let delta_x = interaction.mouse_pos.x - interaction.drag_start_pos.x;
        let candle_width_px = chart.panes[0].space.candle_width_px;
        let candles_moved = -(delta_x / candle_width_px) as i32;

        if candles_moved != 0 {
            // Update shared X-axis state
            let new_start = (chart.visible_candle_start as i32 + candles_moved).max(0) as usize;
            chart.visible_candle_start = new_start.min(
                chart.candles.len().saturating_sub(chart.visible_candle_count)
            );

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart);

            chart.needs_redraw = true;
            interaction.drag_start_pos = interaction.mouse_pos;
        }
    }

    // Zoom: Mouse wheel
    for event in mouse_wheel.read() {
        if mouse_in_pane && !chart.panes.is_empty() {
            let zoom_factor = if event.y > 0.0 { 0.9 } else { 1.1 };

            // Calculate focus candle using first pane
            let (focus_candle, _) = chart.panes[0].space.from_world(
                interaction.mouse_pos,
                chart.visible_candle_start,
                chart.visible_candle_count
            );

            // Update shared X-axis state (zoom)
            let old_count = chart.visible_candle_count;
            let new_count = ((old_count as f32 * zoom_factor).clamp(10.0, 10000.0) as usize)
                .min(chart.candles.len());

            let focus_offset = focus_candle.saturating_sub(chart.visible_candle_start);
            let focus_percent = focus_offset as f32 / old_count as f32;

            chart.visible_candle_start = focus_candle
                .saturating_sub((new_count as f32 * focus_percent) as usize)
                .min(chart.candles.len().saturating_sub(new_count));
            chart.visible_candle_count = new_count;

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart);

            chart.needs_redraw = true;

            println!(
                "Zoomed: showing {} candles starting from {}",
                chart.visible_candle_count,
                chart.visible_candle_start
            );
        }
    }
}

/// Update Y-axis bounds for all panes based on their type
fn update_pane_bounds(chart: &mut Chart) {
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

fn check_lazy_load(
    mut chart: ResMut<Chart>,
    db: Res<ChartDatabase>,
) {
    if chart.loading {
        return;  // Already loading
    }

    let start_idx = chart.visible_candle_start;
    let end_idx = start_idx + chart.visible_candle_count;

    // Load more historical data when scrolling left
    if start_idx < 20 && chart.candles.first().is_some() {
        chart.loading = true;

        let load_count = 100;
        let load_end_time = chart.candles.first().unwrap().time;

        // Calculate start time based on timeframe
        let interval_ms = match chart.timeframe.as_str() {
            "15m" => 15 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            "4h" => 4 * 60 * 60 * 1000,
            "1d" => 24 * 60 * 60 * 1000,
            _ => 60 * 60 * 1000,
        };
        let load_start_time = load_end_time - (load_count as i64 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time,
            load_end_time - 1, // Exclude the first candle we already have
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} historical candles", new_candles.len());

                // Prepend new candles
                let new_len = new_candles.len();
                let mut combined = new_candles;
                combined.append(&mut chart.candles);
                chart.candles = combined;

                // Adjust visible_start to maintain view
                chart.visible_candle_start += new_len;

                chart.needs_redraw = true;
            }
        }

        chart.loading = false;
    }

    // Load more recent data when scrolling right
    if end_idx > chart.candles.len().saturating_sub(20) && chart.candles.last().is_some() {
        chart.loading = true;

        let load_start_time = chart.candles.last().unwrap().time;
        let interval_ms = match chart.timeframe.as_str() {
            "15m" => 15 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            "4h" => 4 * 60 * 60 * 1000,
            "1d" => 24 * 60 * 60 * 1000,
            _ => 60 * 60 * 1000,
        };
        let load_end_time = load_start_time + (100 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time + 1, // Exclude the last candle we already have
            load_end_time,
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} recent candles", new_candles.len());
                chart.candles.extend(new_candles);
                chart.needs_redraw = true;
            }
        }

        chart.loading = false;
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Candlestick Chart".to_string(),
                resolution: (1600.0, 900.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, handle_mouse_input)
        .add_systems(Update, check_lazy_load)
        .add_systems(Update, render_grid_and_axes)
        .add_systems(Update, render_candlesticks)
        .add_systems(Update, render_volume_bars)
        .add_systems(Update, render_crosshair)
        .run();
}
