# Bevy Chart Application - Technical Documentation

**Current Status:** Phase 1 & 2 Complete, Gizmos-based MA rendering implemented

---

## Architecture Overview

### Core Components

**Engine:** Bevy 0.14 with custom features:
- `bevy_winit` - Window management
- `bevy_render` - Rendering pipeline
- `bevy_sprite` - 2D sprite rendering
- `bevy_gizmos` - Direct line rendering for indicators
- `bevy_ui` - Text/UI elements
- VSync disabled (`PresentMode::AutoNoVsync`) for uncapped FPS

**Database:** DuckDB for historical candle data storage

---

## Data Flow

```
Database (DuckDB)
    ↓
Initial Load (500 candles)
    ↓
Chart Resource (in-memory)
    ↓
Rendering Systems (every frame when needs_redraw = true)
    ↓
GPU Display
```

### Lazy Loading

**Trigger:** User pans within 20 candles of edge
**Action:** Load 100 additional candles (prepend or append)
**Optimization:** Incremental MA recalculation (Phase 2)

---

## Rendering Architecture

### Pane System

Multi-pane layout with synchronized X-axis:

```
┌─────────────────────────────────────┐
│  Price Pane (70% height)            │  ← Candlesticks + MAs
│  - Candlesticks (entity-based)      │
│  - MA indicators (Gizmos)           │
├─────────────────────────────────────┤
│  Volume Pane (30% height)           │  ← Volume bars
│  - Volume bars (entity-based)       │
│  - Toggle with 'V' key              │
└─────────────────────────────────────┘
```

**Shared State:**
- `visible_candle_start` - Index of leftmost visible candle
- `visible_candle_count` - Number of candles in viewport
- Each pane has independent Y-axis scaling

### Coordinate System

**ChartSpace:** Converts between data and screen coordinates

```rust
// Data → Screen
let screen_pos = space.to_world(
    candle_index,        // Which candle (0..N)
    price_value,         // Price in dollars
    visible_start,       // Current viewport
    visible_count
);

// Screen → Data
let (candle_idx, price) = space.from_world(
    mouse_pos,
    visible_start,
    visible_count
);
```

---

## Performance Optimizations

### Phase 1: Sliding Window SMA ✅ COMPLETE
**Commit:** 409af8d

**Problem:** Naive O(n × period) calculation
**Solution:** O(n) sliding window algorithm

```rust
// Before: Recalculate sum for every candle
for i in (period-1)..candles.len() {
    let sum: f64 = candles[i-period+1..=i].sum();  // O(period)
}
// 10,000 candles × 200 period = 2M operations

// After: Maintain running sum
let mut sum = initial_sum;
for i in period..candles.len() {
    sum -= candles[i - period].close;  // Remove old
    sum += candles[i].close;           // Add new
}
// 10,000 candles × 2 operations = 20K operations
```

**Performance:** 100-1000× faster depending on period
**Test Coverage:** 7 unit tests covering edge cases and numerical stability

---

### Phase 2: Incremental MA Recalculation ✅ COMPLETE
**Commit:** 9b57096

**Problem:** Recalculating ALL MA values on lazy load
**Solution:** Only calculate new candles

**Append (scrolling right):**
```rust
// Store old length
let old_len = chart.candles.len();

// Extend with new candles
chart.candles.extend(new_candles);

// Only calculate for NEW candles
indicator.calculate_sma_append(&candles, old_len);
```

**Prepend (scrolling left):**
```rust
// Calculate new candles
indicator.calculate_sma_prepend(&candles, new_count);

// Recalculate boundary (period-1 candles affected by prepend)
```

**Performance:** 100× faster lazy loading
**Test Coverage:** 5 unit tests for append/prepend scenarios

---

### Gizmos-Based MA Rendering ✅ COMPLETE
**Commit:** 631cbd4

**Problem:** Rotated sprite rectangles caused visual artifacts at intersections
**Solution:** Use Bevy Gizmos for hardware-accelerated line drawing

```rust
pub fn render_moving_averages(
    mut gizmos: Gizmos,  // No entity management!
    chart: Res<Chart>,
) {
    for ma in &chart.indicators {
        for i in start..(end - 1) {
            // Draw line directly - no entity spawn/despawn
            gizmos.line_2d(curr_pos, next_pos, ma.color);
        }
    }
}
```

**Benefits:**
- No entity spawning/despawning overhead
- Perfect line continuity (no gaps or corner artifacts)
- Simpler code (51 lines → 48 lines)

---

### needs_redraw Flag Reset ✅ COMPLETE
**Commit:** dc5d232

**Critical Bug Fix:** Flag was never reset to false, causing constant entity churn

**Problem:**
```rust
// After ANY interaction:
chart.needs_redraw = true;

// Then EVERY frame:
if chart.needs_redraw {
    despawn_all();
    spawn_all();
}
// Flag never reset → infinite redraw loop!
```

**Solution:**
```rust
.add_systems(Update, (
    render_grid_and_axes,
    render_candlesticks,
    render_moving_averages,
    render_volume_bars,
    reset_redraw_flag,  // ← Reset after rendering
).chain())
```

**Performance:** Prevents 40 FPS drop when zoomed out

---

## Current Rendering Systems

### 1. Candlesticks (Entity-Based) ⚠️ BOTTLENECK

**Status:** Uses despawn/spawn pattern
**Performance:** ~200-400 entity operations per redraw

```rust
pub fn render_candlesticks(
    mut commands: Commands,
    query: Query<Entity, With<PriceElement>>,
) {
    // Despawn ALL
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Spawn NEW for visible range
    for i in start..end {
        commands.spawn(/* wick */);
        commands.spawn(/* body */);
    }
}
```

**Entity Count:** 2 per candle (wick + body)
**Visible Range:** 50-200 candles typical
**Operations per Redraw:** 100-400 entities

---

### 2. Volume Bars (Entity-Based) ⚠️ BOTTLENECK

**Status:** Uses despawn/spawn pattern
**Performance:** ~100-200 entity operations per redraw

```rust
pub fn render_volume_bars(
    mut commands: Commands,
    query: Query<Entity, With<VolumeElement>>,
) {
    // Despawn ALL
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Spawn NEW for visible range
    for i in start..end {
        commands.spawn(/* bar */);
    }
}
```

**Entity Count:** 1 per candle
**Toggleable:** 'V' key to show/hide

---

### 3. MA Indicators (Gizmos) ✅ OPTIMIZED

**Status:** Direct GPU rendering, no entities
**Performance:** No entity overhead

```rust
pub fn render_moving_averages(
    mut gizmos: Gizmos,
    chart: Res<Chart>,
) {
    // Draw directly each frame
    gizmos.line_2d(start_pos, end_pos, color);
}
```

**Indicators:** SMA-20, SMA-50, SMA-200
**Colors:** Yellow, Cyan, Magenta
**Thickness:** 1px (Gizmos default)

---

### 4. Grid & Axes (Entity-Based)

**Status:** Likely despawn/spawn (not optimized)
**Frequency:** Low priority (less visual change)

---

### 5. Crosshair (Persistent Entities) ✅ OPTIMIZED

**Status:** Uses persistent entity pattern
**Implementation:** Created once at startup, updated each frame

```rust
pub struct CrosshairEntities {
    pub vertical_line_segments: Vec<Entity>,
    pub horizontal_lines: Vec<(PaneId, Vec<Entity>)>,
    pub price_labels: Vec<(PaneId, Entity)>,
    pub time_label: Entity,
    pub ohlcv_box: Entity,
}
```

**Performance:** Constant entity count, update-only

---

## Known Performance Issues

### 1. Resize Lag ⚠️ CURRENT BOTTLENECK

**Symptom:** FPS drops from 100 to 40 during aggressive resize bar dragging

**Root Cause:**
```
Mouse movement at 120 Hz
    ↓
needs_redraw = true (every mouse event)
    ↓
Despawn 200 candlestick entities
Despawn 100 volume bar entities
Spawn 200 new candlestick entities
Spawn 100 new volume bar entities
    ↓
= 600 entity operations × 120 Hz = 72,000 ops/sec
```

**Solution:** Apply persistent entity pattern to candlesticks and volume bars (Phase 3 concept)

---

### 2. Entity Count Scaling

**Current State:**
```
50 candles visible:
- Candlesticks: 100 entities (50 wicks + 50 bodies)
- Volume: 50 entities
- Total: 150 despawn/spawn per redraw

200 candles visible:
- Candlesticks: 400 entities
- Volume: 200 entities
- Total: 600 despawn/spawn per redraw
```

**Problem:** Linear scaling with visible candle count

---

## User Interactions

### Pan (Arrow Keys or Mouse Drag)

**Action:** Moves viewport left/right
**Effect:** Updates `visible_candle_start`
**Lazy Load:** Triggers when within 20 candles of edge
**Performance:** Incremental MA calculation (Phase 2)

---

### Zoom (Mouse Wheel)

**Action:** Changes number of visible candles
**Effect:** Updates `visible_candle_count`
**Range:** ~10 to 500 candles
**Performance:** Scales entity operations linearly

---

### Resize Panes (Drag Gap)

**Action:** Adjusts height ratio between panes
**Trigger:** Mouse hover + drag on separator
**Performance Issue:** Sets `needs_redraw = true` on every mouse pixel movement

**Code Path:**
```rust
// interaction.rs:73-108
if let Some(gap_idx) = interaction.resizing_gap {
    // On EVERY mouse movement:
    chart.needs_redraw = true;  // ← High frequency
}
```

---

### Toggle Volume ('V' Key)

**Action:** Show/hide volume pane
**Implementation:** Recalculates pane layouts
**Performance:** Single redraw, not a bottleneck

---

## FPS Counter

**Location:** Top-right corner
**Color:** Green
**Update:** Every frame
**Source:** Bevy's `FrameTimeDiagnosticsPlugin` (smoothed)

**Display:** "FPS: 100" (rounded to integer)

---

## Data Structures

### Chart (Main Resource)

```rust
pub struct Chart {
    pub ticker_id: i32,
    pub timeframe: String,
    pub candles: Vec<Candle>,
    pub visible_candle_start: usize,
    pub visible_candle_count: usize,
    pub panes: Vec<Pane>,
    pub indicators: Vec<MovingAverage>,
    pub needs_redraw: bool,
    pub loading: bool,
}
```

---

### Candle (Price Data)

```rust
pub struct Candle {
    pub time: i64,      // Unix timestamp (ms)
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
```

---

### MovingAverage (Indicator)

```rust
pub struct MovingAverage {
    pub period: usize,              // 20, 50, 200
    pub values: Vec<Option<f32>>,   // Cached MA per candle
    pub name: String,               // "SMA-20"
    pub color: Color,
    pub visible: bool,
}
```

**Methods:**
- `calculate_sma()` - Full calculation (O(n))
- `calculate_sma_append()` - Incremental for new candles (right)
- `calculate_sma_prepend()` - Incremental for historical (left)
- `calculate_ema()` - Exponential MA (still full recalc)

---

### Pane (Viewport Region)

```rust
pub struct Pane {
    pub id: PaneId,               // Price or Volume
    pub pane_type: PaneType,
    pub height_percent: f32,      // 0.7 for price, 0.3 for volume
    pub space: ChartSpace,        // Coordinate conversion
}
```

---

## File Structure

```
charts/
├── src/
│   ├── main.rs           - App setup, FPS counter, redraw flag
│   ├── types.rs          - Data structures, MA calculations
│   ├── rendering.rs      - All rendering systems
│   └── interaction.rs    - Mouse/keyboard input, lazy load
├── Cargo.toml            - Dependencies
├── PHASE_1.md            - Sliding window SMA plan
├── PHASE_2.md            - Incremental MA plan
├── PHASE_3.md            - Persistent entities plan (for indicators - now obsolete)
└── DOCS.md               - This file
```

---

## Test Coverage

### Unit Tests (src/types.rs)

**SMA Sliding Window (7 tests):**
- `test_sma_sliding_window_matches_naive` - Correctness vs old implementation
- `test_sma_different_periods` - Multiple period sizes (5, 10, 20, 50, 100, 200)
- `test_sma_edge_case_empty_candles` - Empty input
- `test_sma_edge_case_insufficient_candles` - Less than period
- `test_sma_edge_case_exact_period` - Exactly period candles
- `test_sma_edge_case_period_one` - Period = 1
- `test_sma_numerical_stability` - Large numbers

**SMA Incremental (5 tests):**
- `test_sma_append_incremental` - Verify append correctness
- `test_sma_prepend_incremental` - Verify prepend correctness
- `test_sma_append_boundary_conditions` - Single candle append
- `test_sma_prepend_insufficient_period` - Not enough prepended candles
- `test_sma_append_different_periods` - Multiple periods

**Total:** 12 passing tests

---

## Optimization Roadmap

### Completed ✅

1. **Phase 1:** Sliding window SMA (100-1000× faster calculation)
2. **Phase 2:** Incremental MA recalculation (100× faster lazy load)
3. **Gizmos MA rendering:** No entity overhead, perfect lines
4. **needs_redraw reset:** Prevents infinite redraw loop
5. **FPS counter:** Performance monitoring

### Pending ⚠️

1. **Persistent Candlestick Entities:**
   - Apply Phase 3 pattern to candlesticks
   - Dynamic pool (grows with zoom)
   - Would fix resize lag

2. **Persistent Volume Bar Entities:**
   - Apply Phase 3 pattern to volume bars
   - Fixed pool (500 entities sufficient)
   - Complements candlestick optimization

3. **Throttle Resize Redraws:**
   - Debounce or throttle `needs_redraw` during resize
   - Skip some frames during rapid mouse movement

---

## Performance Metrics

### Current State (with optimizations)

**FPS (uncapped):**
- Idle: ~100 FPS (likely compositor/driver cap)
- Normal pan/zoom: 60-100 FPS
- Aggressive resize: 40-60 FPS ⚠️

**Entity Operations:**
- Static view: 0 operations/frame
- Single pan: ~300 entity operations (1 frame)
- Aggressive resize: ~600 operations × 120 Hz = 72K ops/sec

**Memory:**
- ~10,000 candles loaded: ~1 MB
- Entity overhead: Varies with visible count (100-600 entities)
- Total: ~5-10 MB

---

## Known Limitations

1. **Fixed indicator count:** 3 SMAs (20, 50, 200) hardcoded
2. **EMA incremental:** Not implemented, falls back to full recalculation
3. **Max zoom:** No enforced limit, but >500 candles untested
4. **Entity churn:** Candlesticks and volume still use despawn/spawn
5. **Grid rendering:** Not optimized (low priority)

---

## Future Improvements

### Short Term
- Persistent entities for candlesticks/volume
- Throttle resize redraws
- Add EMA incremental calculation

### Medium Term
- Dynamic indicator add/remove UI
- Multiple timeframe support
- Save/load chart configurations

### Long Term
- GPU instancing for massive datasets
- WebGPU rendering backend
- Real-time data streaming

---

## Build & Run

```bash
# Development build
cargo build
cargo run

# Release build (recommended)
cargo build --release
cargo run --release

# Run tests
cargo test

# Run specific test
cargo test test_sma_append
```

---

## Dependencies

```toml
[dependencies]
bevy = { version = "0.14", features = [
    "bevy_winit",
    "bevy_render",
    "bevy_core_pipeline",
    "bevy_sprite",
    "bevy_text",
    "bevy_ui",
    "bevy_gizmos",      # For MA line rendering
    "default_font",
    "png",
    "wayland",
    "x11",
] }
duckdb = "1.1"
chrono = "0.4"
```

---

## Recent Commits

```
dc5d232 - Fix critical performance bug: reset needs_redraw flag
0928bf8 - Disable VSync for uncapped FPS display
a84d90e - Add FPS counter to top-right corner
9b57096 - Implement incremental MA recalculation on lazy load
631cbd4 - Fix MA line intersection artifacts by switching to Bevy Gizmos
409af8d - Optimize SMA calculation with sliding window algorithm
```

---

**Last Updated:** 2025-11-04
**Version:** Phase 1 & 2 Complete
**Performance Status:** Good (with known resize bottleneck)
