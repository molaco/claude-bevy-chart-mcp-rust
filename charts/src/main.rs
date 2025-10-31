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
    // X-axis: Time/Candle axis (logical)
    visible_candle_start: usize,   // First visible candle index
    visible_candle_count: usize,   // How many candles visible

    // Y-axis: Price axis (logical)
    visible_price_min: f32,        // Auto-calculated from visible candles
    visible_price_max: f32,        // Auto-calculated from visible candles
    price_padding: f32,            // Extra space above/below (5-10%)

    // Physical viewport (Bevy world space)
    viewport: Rect,                // Where chart is drawn on screen

    // Cached calculations (updated when bounds change)
    candle_width_px: f32,          // Width of each candle in pixels
    price_scale: f32,              // Pixels per $1 price movement
}

impl ChartSpace {
    fn new(viewport: Rect, visible_candle_count: usize) -> Self {
        Self {
            visible_candle_start: 0,
            visible_candle_count,
            visible_price_min: 0.0,
            visible_price_max: 100.0,
            price_padding: 0.05,
            viewport,
            candle_width_px: 0.0,
            price_scale: 0.0,
        }
    }

    /// Map candle index and price to world coordinates
    fn to_world(&self, candle_index: usize, price: f32) -> Vec2 {
        // Map candle index relative to visible range
        let candle_offset = candle_index.saturating_sub(self.visible_candle_start);
        let x_percent = candle_offset as f32 / self.visible_candle_count as f32;
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
    fn from_world(&self, world_pos: Vec2) -> (usize, f32) {
        let x_percent = (world_pos.x - self.viewport.min.x) / self.viewport.width();
        let candle_index = self.visible_candle_start +
            (x_percent * self.visible_candle_count as f32) as usize;

        let y_percent = (world_pos.y - self.viewport.min.y) / self.viewport.height();
        let price = self.visible_price_min +
            y_percent * (self.visible_price_max - self.visible_price_min);

        (candle_index, price)
    }

    /// Zoom while keeping focus candle at same screen position
    fn zoom(&mut self, factor: f32, focus_candle_index: usize, total_candles: usize) {
        let old_count = self.visible_candle_count;
        let new_count = ((old_count as f32 * factor).clamp(10.0, 10000.0) as usize)
            .min(total_candles);

        let focus_offset = focus_candle_index.saturating_sub(self.visible_candle_start);
        let focus_percent = focus_offset as f32 / old_count as f32;

        self.visible_candle_start = focus_candle_index
            .saturating_sub((new_count as f32 * focus_percent) as usize)
            .min(total_candles.saturating_sub(new_count));
        self.visible_candle_count = new_count;

        self.recalculate_cache();
    }

    /// Pan by candle delta
    fn pan(&mut self, candle_delta: i32, total_candles: usize) {
        let new_start = (self.visible_candle_start as i32 + candle_delta)
            .max(0) as usize;
        self.visible_candle_start = new_start.min(
            total_candles.saturating_sub(self.visible_candle_count)
        );
        self.recalculate_cache();
    }

    /// Fit price bounds to visible candles
    fn fit_price_bounds(&mut self, candles: &[Candle]) {
        if candles.is_empty() {
            return;
        }

        let start = self.visible_candle_start;
        let end = (start + self.visible_candle_count).min(candles.len());

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

        self.recalculate_cache();
    }

    /// Helper to fit price bounds using a mutable reference
    fn update_price_bounds(&mut self, candles: &[Candle]) {
        self.fit_price_bounds(candles);
    }

    /// Recalculate cached values
    fn recalculate_cache(&mut self) {
        self.candle_width_px = self.viewport.width() / self.visible_candle_count as f32;
        let price_range = self.visible_price_max - self.visible_price_min;
        self.price_scale = if price_range > 0.0 {
            self.viewport.height() / price_range
        } else {
            1.0
        };
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

    // Coordinate system
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

    // Initialize chart space
    let viewport = Rect::from_center_size(
        Vec2::ZERO,
        Vec2::new(1600.0, 900.0),
    );

    let visible_candle_count = 50.min(candles.len());
    let mut space = ChartSpace::new(viewport, visible_candle_count);

    // Start at the end of the data (most recent candles)
    space.visible_candle_start = candles.len().saturating_sub(visible_candle_count);
    space.fit_price_bounds(&candles);

    let chart = Chart {
        ticker_id,
        timeframe: timeframe.to_string(),
        candles,
        candle_offset: 0,
        space,
        needs_redraw: true,
        loading: false,
    };

    commands.insert_resource(db);
    commands.insert_resource(chart);
    commands.insert_resource(InteractionState::default());

    println!("Setup complete!");
}

fn render_candlesticks(
    mut commands: Commands,
    mut chart: ResMut<Chart>,
    query: Query<Entity, With<ChartElement>>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn all existing chart elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Render visible candles
    let start = chart.space.visible_candle_start;
    let end = (start + chart.space.visible_candle_count).min(chart.candles.len());

    if start >= end {
        chart.needs_redraw = false;
        return;
    }

    for i in start..end {
        let candle = &chart.candles[i];

        // Calculate positions using ChartSpace::to_world()
        let wick_bottom = chart.space.to_world(i, candle.low as f32);
        let wick_top = chart.space.to_world(i, candle.high as f32);
        let body_open = chart.space.to_world(i, candle.open as f32);
        let body_close = chart.space.to_world(i, candle.close as f32);

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
        ));

        // Spawn body entity (rectangle, Z=1 above wick)
        let body_width = chart.space.candle_width_px * 0.7;
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
        ));
    }

    chart.needs_redraw = false;

    println!(
        "Rendered {} candles (indices {}-{})",
        end - start,
        start,
        end - 1
    );
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
    if let Some(cursor_pos) = window.cursor_position() {
        let window_size = Vec2::new(window.width(), window.height());
        interaction.mouse_pos = cursor_pos - window_size / 2.0;
    }

    // Pan: Left mouse button drag
    if mouse_button.just_pressed(MouseButton::Left) {
        if chart.space.viewport.contains(interaction.mouse_pos) {
            interaction.dragging = true;
            interaction.drag_start_pos = interaction.mouse_pos;
        }
    }

    if mouse_button.just_released(MouseButton::Left) {
        interaction.dragging = false;
    }

    if interaction.dragging {
        let delta_x = interaction.mouse_pos.x - interaction.drag_start_pos.x;
        let candles_moved = -(delta_x / chart.space.candle_width_px) as i32;

        if candles_moved != 0 {
            let candles_len = chart.candles.len();
            chart.space.pan(candles_moved, candles_len);

            // Calculate price bounds
            let start = chart.space.visible_candle_start;
            let end = (start + chart.space.visible_candle_count).min(chart.candles.len());
            if start < end {
                let visible_candles = &chart.candles[start..end];
                let mut min_price = f64::MAX;
                let mut max_price = f64::MIN;
                for candle in visible_candles {
                    min_price = min_price.min(candle.low);
                    max_price = max_price.max(candle.high);
                }
                let range = max_price - min_price;
                let padding = range * chart.space.price_padding as f64;
                chart.space.visible_price_min = (min_price - padding) as f32;
                chart.space.visible_price_max = (max_price + padding) as f32;
                chart.space.recalculate_cache();
            }

            chart.needs_redraw = true;
            interaction.drag_start_pos = interaction.mouse_pos;
        }
    }

    // Zoom: Mouse wheel
    for event in mouse_wheel.read() {
        if chart.space.viewport.contains(interaction.mouse_pos) {
            let zoom_factor = if event.y > 0.0 { 0.9 } else { 1.1 };

            let (focus_candle, _) = chart.space.from_world(interaction.mouse_pos);
            let candles_len = chart.candles.len();
            chart.space.zoom(zoom_factor, focus_candle, candles_len);

            // Calculate price bounds
            let start = chart.space.visible_candle_start;
            let end = (start + chart.space.visible_candle_count).min(chart.candles.len());
            if start < end {
                let visible_candles = &chart.candles[start..end];
                let mut min_price = f64::MAX;
                let mut max_price = f64::MIN;
                for candle in visible_candles {
                    min_price = min_price.min(candle.low);
                    max_price = max_price.max(candle.high);
                }
                let range = max_price - min_price;
                let padding = range * chart.space.price_padding as f64;
                chart.space.visible_price_min = (min_price - padding) as f32;
                chart.space.visible_price_max = (max_price + padding) as f32;
                chart.space.recalculate_cache();
            }

            chart.needs_redraw = true;

            println!(
                "Zoomed: showing {} candles starting from {}",
                chart.space.visible_candle_count,
                chart.space.visible_candle_start
            );
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

    let start_idx = chart.space.visible_candle_start;
    let end_idx = start_idx + chart.space.visible_candle_count;

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
                let old_len = combined.len() - new_len;
                chart.candles = combined;

                // Adjust visible_start to maintain view
                chart.space.visible_candle_start += new_len;

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
        .add_systems(Update, render_candlesticks)
        .run();
}
