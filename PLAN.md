# Candlestick Chart Architecture Plan

## Project: bevy-chart-2
**Goal:** Build a high-performance candlestick chart with panes and indicators using Bevy Rust game engine

---

## 📚 Reference Analysis

### tradingview-rs
- **Type:** Data library (not rendering)
- **Useful patterns:**
  - OHLCV trait abstraction
  - Session/pane management concepts
  - Indicator architecture (Pine Script integration)
  - Multi-session metadata handling with DashMap
- **Not useful:** No rendering implementation

### bevy_plot
- **Type:** Scientific plotting library
- **Useful patterns:**
  - GPU-accelerated rendering (WGSL shaders)
  - Interaction system (pan, zoom, crosshair)
  - Material2d rendering pipeline
  - Event-driven updates
  - Coordinate transformation system
- **Limitations:** No candlestick support, no multi-pane stacking, designed for scientific plots

---

## 🗄️ Database Structure

**Location:** `/home/molaco/.local/share/flowsurface/flowsurface.duckdb`

### klines table
```sql
- kline_id: BIGINT (PK)
- ticker_id: INTEGER → links to tickers.ticker_id
- timeframe: VARCHAR ('15m', '1h')
- candle_time: BIGINT (Unix timestamp in milliseconds)
- open_price: DECIMAL(18,8)
- high_price: DECIMAL(18,8)
- low_price: DECIMAL(18,8)
- close_price: DECIMAL(18,8)
- volume: DECIMAL(18,8)
- num_trades: INTEGER
```

**Current data:**
- 568 klines total
- Symbols: BTCUSDT, ETHUSDT (LinearPerps)
- Timeframes: 15m (454), 1h (114)
- Date range: Oct 24-31, 2025

---

## 🎯 Coordinate System: Hybrid Approach

### Three Spaces

```
Real Data Space          Logical Chart Space        Physical World Space
(prices, timestamps)  →  (candle indices, prices) → (Bevy pixel coordinates)
```

### ChartSpace Structure

```rust
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
```

### Key Operations

**Data → World:**
```rust
fn to_world(&self, candle_index: usize, price: f32) -> Vec2 {
    // Map candle index relative to visible range
    let candle_offset = candle_index - self.visible_candle_start;
    let x_percent = candle_offset as f32 / self.visible_candle_count as f32;
    let world_x = self.viewport.min.x + x_percent * self.viewport.width();

    // Map price to viewport
    let price_percent = (price - self.visible_price_min) /
                        (self.visible_price_max - self.visible_price_min);
    let world_y = self.viewport.min.y + price_percent * self.viewport.height();

    Vec2::new(world_x, world_y)
}
```

**World → Data:**
```rust
fn from_world(&self, world_pos: Vec2) -> (usize, f32) {
    let x_percent = (world_pos.x - self.viewport.min.x) / self.viewport.width();
    let candle_index = self.visible_candle_start +
        (x_percent * self.visible_candle_count as f32) as usize;

    let y_percent = (world_pos.y - self.viewport.min.y) / self.viewport.height();
    let price = self.visible_price_min +
        y_percent * (self.visible_price_max - self.visible_price_min);

    (candle_index, price)
}
```

**Zoom:**
```rust
fn zoom(&mut self, factor: f32, focus_candle_index: usize) {
    // Change visible_candle_count while keeping focus_candle at same screen position
    let old_count = self.visible_candle_count;
    let new_count = (old_count as f32 * factor).clamp(10.0, 10000.0) as usize;

    let focus_offset = focus_candle_index - self.visible_candle_start;
    let focus_percent = focus_offset as f32 / old_count as f32;

    self.visible_candle_start = focus_candle_index
        .saturating_sub((new_count as f32 * focus_percent) as usize);
    self.visible_candle_count = new_count;

    self.recalculate_cache();
}
```

**Pan:**
```rust
fn pan(&mut self, candle_delta: i32) {
    self.visible_candle_start = (self.visible_candle_start as i32 + candle_delta)
        .max(0) as usize;
    self.recalculate_cache();
}
```

### Why Hybrid?

✅ **Natural for trading charts:** "Show me 50 candles starting from index 1000"
✅ **Intuitive interactions:** Zoom = show more/fewer candles, Pan = scroll in time
✅ **Easy lazy loading:** When `visible_candle_start < 10`, load more historical data
✅ **Multi-pane syncing:** All panes share same `visible_candle_start/count`
✅ **Clean separation:** Chart logic (indices, prices) vs rendering (pixels)

---

## 📋 PHASE 1: Basic Candlestick Chart

### Core Data Structures

```rust
/// Raw candle from database
struct Candle {
    time: i64,           // Unix timestamp in ms
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
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

/// Database connection resource
#[derive(Resource)]
struct ChartDatabase {
    conn: duckdb::Connection,
}

/// Interaction state
#[derive(Resource, Default)]
struct InteractionState {
    mouse_pos: Vec2,
    dragging: bool,
    drag_start_pos: Vec2,
    drag_start_candle: usize,
}
```

### Components

```rust
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
```

---

## 🎨 Rendering Strategy

**Approach:** Bevy 2D primitives using Sprite components

### Candlestick Visual Structure

```
     │        Each candlestick = 2 entities:
     │        1. Wick (thin vertical line from low to high)
  ┌──┴──┐     2. Body (rectangle from open to close)
  │     │
  │     │     Colors:
  │     │     - Bullish (close > open): Green body
  └──┬──┘     - Bearish (close < open): Red body
     │        - Doji (close ≈ open): Gray thin line
     │
```

### Rendering System

```rust
fn render_candlesticks(
    mut commands: Commands,
    chart: Res<Chart>,
    query: Query<Entity, With<ChartElement>>,
) {
    // 1. Despawn all existing chart elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // 2. Render visible candles
    let start = chart.space.visible_candle_start;
    let end = (start + chart.space.visible_candle_count).min(chart.candles.len());

    for i in start..end {
        let candle = &chart.candles[i];
        let candle_index = i - start;

        // Calculate positions using ChartSpace::to_world()
        let wick_bottom = chart.space.to_world(candle_index, candle.low as f32);
        let wick_top = chart.space.to_world(candle_index, candle.high as f32);
        let body_open = chart.space.to_world(candle_index, candle.open as f32);
        let body_close = chart.space.to_world(candle_index, candle.close as f32);

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
}
```

---

## 💾 Database Integration

### Database Operations

```rust
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

        let mut stmt = self.conn.prepare(query)?;
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
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*) FROM klines WHERE ticker_id = ? AND timeframe = ?"
        )?;
        let count: i64 = stmt.query_row(
            params![ticker_id, timeframe],
            |row| row.get(0)
        )?;
        Ok(count as usize)
    }
}
```

### Lazy Loading System

```rust
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
    if start_idx < 20 && chart.candle_offset > 0 {
        chart.loading = true;

        let load_count = 100;
        let load_end_time = chart.candles.first().map(|c| c.time).unwrap_or(i64::MAX);

        // Calculate start time based on timeframe
        let interval_ms = match chart.timeframe.as_str() {
            "15m" => 15 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            _ => 60 * 60 * 1000,
        };
        let load_start_time = load_end_time - (load_count as i64 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time,
            load_end_time,
        ) {
            // Prepend new candles
            let mut combined = new_candles;
            combined.append(&mut chart.candles);
            chart.candles = combined;

            // Adjust visible_start to maintain view
            chart.space.visible_candle_start += new_candles.len();
            chart.candle_offset = chart.candle_offset.saturating_sub(new_candles.len());
        }

        chart.loading = false;
        chart.needs_redraw = true;
    }

    // Load more recent data when scrolling right
    if end_idx > chart.candles.len().saturating_sub(20) {
        chart.loading = true;

        let load_start_time = chart.candles.last().map(|c| c.time).unwrap_or(0);
        let interval_ms = match chart.timeframe.as_str() {
            "15m" => 15 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            _ => 60 * 60 * 1000,
        };
        let load_end_time = load_start_time + (100 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time,
            load_end_time,
        ) {
            chart.candles.extend(new_candles);
        }

        chart.loading = false;
        chart.needs_redraw = true;
    }
}
```

---

## 🖱️ Interaction System

### Mouse Input Handling

```rust
fn handle_mouse_input(
    mut chart: ResMut<Chart>,
    mut interaction: ResMut<InteractionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: EventReader<MouseWheel>,
    window_query: Query<&Window>,
) {
    let window = window_query.single();

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

        chart.space.pan(candles_moved);
        chart.needs_redraw = true;

        interaction.drag_start_pos = interaction.mouse_pos;
    }

    // Zoom: Mouse wheel
    for event in mouse_wheel.read() {
        if chart.space.viewport.contains(interaction.mouse_pos) {
            let zoom_factor = if event.y > 0.0 { 0.9 } else { 1.1 };

            let (focus_candle, _) = chart.space.from_world(interaction.mouse_pos);
            chart.space.zoom(zoom_factor, focus_candle);
            chart.space.fit_price_bounds(&chart.candles);
            chart.needs_redraw = true;
        }
    }
}
```

---

## 🔧 System Schedule

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            handle_mouse_input,
            check_lazy_load,
            render_candlesticks.run_if(|chart: Res<Chart>| chart.needs_redraw),
        ).chain())
        .run();
}

fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn(Camera2dBundle::default());

    // Initialize database connection
    let conn = duckdb::Connection::open(
        "/home/molaco/.local/share/flowsurface/flowsurface.duckdb"
    ).expect("Failed to open database");

    commands.insert_resource(ChartDatabase { conn });

    // Initialize chart with data from database
    // ... (see implementation details below)

    commands.insert_resource(InteractionState::default());
}
```

---

## 📦 Dependencies

```toml
[dependencies]
bevy = "0.14"
duckdb = "1.1"
chrono = "0.4"  # For timestamp formatting (Phase 2+)
```

---

## 🔮 PHASE 2+: Future Expansion

### Multi-Pane Layout

```rust
struct Pane {
    id: PaneId,
    pane_type: PaneType,
    height_percent: f32,        // % of total chart height
    space: ChartSpace,          // Own Y-axis, shares X-axis with other panes
}

enum PaneType {
    Price,                      // Main candlestick chart
    Volume,                     // Volume bars
    Indicator { name: String }, // RSI, MACD, etc.
}

// All panes share:
// - visible_candle_start
// - visible_candle_count
// Each pane has independent:
// - visible_price_min/max (Y-axis bounds)
// - viewport.y and viewport.height
```

### Overlays (on price chart)

```rust
trait Overlay {
    fn name(&self) -> &str;
    fn calculate(&self, candles: &[Candle]) -> Vec<(usize, f32)>;
    fn render(&self, points: &[(usize, f32)], space: &ChartSpace, commands: &mut Commands);
}

// Examples:
struct MovingAverage { period: usize, color: Color }
struct BollingerBands { period: usize, std_dev: f32 }
struct VWAP { /* ... */ }
```

### Indicators (separate panes)

```rust
trait Indicator {
    fn name(&self) -> &str;
    fn calculate(&self, candles: &[Candle]) -> IndicatorData;
    fn preferred_height(&self) -> f32;
    fn y_bounds(&self) -> (f32, f32);  // e.g., RSI: (0, 100)
}

// Examples:
struct RSI { period: usize }
struct MACD { fast: usize, slow: usize, signal: usize }
struct Stochastic { /* ... */ }
```

### Grid & Axes

```rust
struct ChartGrid {
    show_grid: bool,
    grid_color: Color,
    x_tick_interval: Duration,
    y_tick_interval: f32,
}

struct ChartAxes {
    show_x_labels: bool,
    show_y_labels: bool,
    font: Handle<Font>,
    label_color: Color,
}
```

### Crosshair & Info Panel

```rust
struct Crosshair {
    enabled: bool,
    color: Color,
    show_ohlcv_box: bool,  // Show OHLCV info at crosshair
}
```

### Real-time Updates

```rust
// WebSocket or polling for new candles
fn update_realtime_data(
    mut chart: ResMut<Chart>,
    // ... real-time data source
) {
    // Append new candles as they arrive
    // Update visible range if following latest data
}
```

---

## ✅ Phase 1 Deliverables

**Core Features:**
- ✅ Basic candlestick rendering (wicks + bodies, bullish/bearish colors)
- ✅ Hybrid coordinate system (logical chart space → Bevy world space)
- ✅ Pan interaction (left mouse drag)
- ✅ Zoom interaction (mouse wheel, centered on cursor)
- ✅ Lazy loading from DuckDB (load more data when scrolling)
- ✅ Auto-fit price axis to visible candles
- ✅ Clean architecture ready for expansion

**Architecture Benefits:**
- 🎯 **Separation of concerns:** ChartSpace (logic) vs rendering (Bevy entities)
- 🔄 **Extensible:** Easy to add panes, overlays, indicators later
- ⚡ **Performant:** Only render visible candles, lazy load on demand
- 🧩 **Modular:** Each system has clear, single responsibility

---

## 📝 Open Questions for Phase 1

1. **Initial viewport size:** 800x600 pixels or fullscreen?
2. **Default visible candles:** Start showing 50 candles?
3. **Candlestick styling:**
   - Bullish: Green (#00CC33)?
   - Bearish: Red (#E63939)?
   - Wick: Gray (#808080)?
4. **Background:** Dark theme (black/dark gray) or light?
5. **Debug UI:** Add FPS counter and chart metrics display?
6. **Initial timeframe:** Default to "15m" or "1h"?
7. **Initial ticker:** BTCUSDT (ticker_id=1)?

---

## 🚀 Next Steps

1. Set up project dependencies in `Cargo.toml`
2. Implement core data structures (Candle, Chart, ChartSpace, etc.)
3. Implement ChartSpace coordinate transformation methods
4. Set up database connection and query functions
5. Implement basic rendering system (candlesticks only)
6. Implement interaction system (mouse input)
7. Implement lazy loading system
8. Test and iterate on Phase 1
9. Plan Phase 2 (multi-pane, overlays, indicators)

---

## 📐 System Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    Bevy App                              │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │            Startup System                          │ │
│  │  - Spawn Camera2D                                  │ │
│  │  - Initialize ChartDatabase                        │ │
│  │  - Load initial candles                            │ │
│  │  - Initialize Chart resource                       │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │            Update Systems (chain)                  │ │
│  │                                                    │ │
│  │  1. handle_mouse_input                            │ │
│  │     - Pan: Update visible_candle_start            │ │
│  │     - Zoom: Update visible_candle_count           │ │
│  │     - Set needs_redraw = true                     │ │
│  │                                                    │ │
│  │  2. check_lazy_load                               │ │
│  │     - Check if near edges                         │ │
│  │     - Query database for more candles             │ │
│  │     - Append/prepend to Chart.candles             │ │
│  │     - Set needs_redraw = true                     │ │
│  │                                                    │ │
│  │  3. render_candlesticks (conditional)             │ │
│  │     - Despawn old entities                        │ │
│  │     - Spawn new candlestick entities              │ │
│  │     - Use ChartSpace::to_world() for positions    │ │
│  │     - Reset needs_redraw = false                  │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│                 Resources                                │
├─────────────────────────────────────────────────────────┤
│ Chart                                                    │
│  ├─ ticker_id, timeframe                                │
│  ├─ candles: Vec<Candle>                                │
│  ├─ space: ChartSpace                                   │
│  │   ├─ visible_candle_start, visible_candle_count     │
│  │   ├─ visible_price_min, visible_price_max           │
│  │   ├─ viewport: Rect                                  │
│  │   └─ cached: candle_width_px, price_scale           │
│  └─ needs_redraw, loading                               │
├─────────────────────────────────────────────────────────┤
│ ChartDatabase                                            │
│  └─ conn: duckdb::Connection                            │
├─────────────────────────────────────────────────────────┤
│ InteractionState                                         │
│  ├─ mouse_pos: Vec2                                     │
│  ├─ dragging: bool                                      │
│  └─ drag_start_pos, drag_start_candle                   │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│              Entities (spawned/despawned)                │
├─────────────────────────────────────────────────────────┤
│ Camera2dBundle                                           │
├─────────────────────────────────────────────────────────┤
│ Candlestick Entities (per visible candle):              │
│  ├─ CandlestickWick                                     │
│  │   └─ SpriteBundle (thin line, Z=0)                   │
│  └─ CandlestickBody                                     │
│      └─ SpriteBundle (rectangle, Z=1)                   │
└─────────────────────────────────────────────────────────┘
```
