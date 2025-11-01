# Bevy Chart Documentation

## Overview

A high-performance candlestick chart built with Bevy game engine featuring multi-pane layout, real-time interactions, and lazy loading from DuckDB.

## Architecture

### Multi-Pane System

The chart uses a **hybrid coordinate system** with:
- **Shared X-axis**: All panes show the same time range and candle indices
- **Independent Y-axes**: Each pane has its own value range (price, volume, indicators)

```
┌─────────────────────────────────────┐
│   Price Pane (70%)                  │ ← Price Y-axis
│   Candlesticks                      │
├─────────────────────────────────────┤ ← Pane Separator
│   Volume Pane (30%)                 │ ← Volume Y-axis
│   Volume Bars                       │
└─────────────────────────────────────┘
         Shared Time X-axis
```

### Core Data Structures

#### `Candle`
Raw OHLCV data from database:
```rust
struct Candle {
    time: i64,      // Unix timestamp in ms
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}
```

#### `ChartSpace`
Pane-specific coordinate transformation:
```rust
struct ChartSpace {
    // Y-axis (pane-specific)
    visible_price_min: f32,
    visible_price_max: f32,
    price_padding: f32,

    // Physical viewport
    viewport: Rect,

    // Cached calculations
    candle_width_px: f32,
    price_scale: f32,
}
```

Key methods:
- `to_world(candle_index, price, visible_candle_start, visible_candle_count) -> Vec2`: Convert data coordinates to screen coordinates
- `from_world(world_pos, visible_candle_start, visible_candle_count) -> (usize, f32)`: Convert screen coordinates to data coordinates
- `fit_price_bounds(candles, visible_candle_start, visible_candle_count)`: Auto-scale Y-axis to visible data
- `fit_volume_bounds(candles, visible_candle_start, visible_candle_count)`: Auto-scale Y-axis for volume

#### `Pane`
Individual pane configuration:
```rust
struct Pane {
    id: PaneId,              // Price, Volume, or Indicator(n)
    pane_type: PaneType,     // What content to render
    height_percent: f32,     // % of total chart height (0.0-1.0)
    space: ChartSpace,       // Coordinate system for this pane
}
```

#### `Chart`
Main resource managing the entire chart:
```rust
struct Chart {
    // Data
    candles: Vec<Candle>,

    // Shared X-axis (synchronized across all panes)
    visible_candle_start: usize,
    visible_candle_count: usize,

    // Multi-pane support
    panes: Vec<Pane>,

    // State
    needs_redraw: bool,
    loading: bool,
}
```

### Coordinate Transformation

#### Data Space → World Space
```rust
// Example: Render a candlestick at index 100 with high price of 50000.0
let price_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price)).unwrap();
let world_pos = price_pane.space.to_world(
    100,                              // candle_index
    50000.0,                          // price
    chart.visible_candle_start,       // shared X-axis state
    chart.visible_candle_count        // shared X-axis state
);
// world_pos is now in Bevy world coordinates (pixels)
```

#### World Space → Data Space
```rust
// Example: Find what candle and price the mouse is hovering over
let (candle_index, price) = price_pane.space.from_world(
    mouse_pos,                        // Vec2 in world coordinates
    chart.visible_candle_start,       // shared X-axis state
    chart.visible_candle_count        // shared X-axis state
);
```

## Rendering Systems

### `render_candlesticks`
Renders OHLC candlesticks in the Price pane.

**Frequency**: Every frame when `chart.needs_redraw == true`

**Process**:
1. Despawn all existing chart elements
2. Find the Price pane
3. For each visible candle:
   - Calculate wick positions (low to high)
   - Calculate body positions (open to close)
   - Spawn wick sprite (thin line, Z=0)
   - Spawn body sprite (colored rectangle, Z=1)
   - Color: Green if close ≥ open, Red otherwise

### `render_volume_bars`
Renders volume bars in the Volume pane.

**Frequency**: Every frame when `chart.needs_redraw == true`

**Process**:
1. Despawn all existing volume bar elements
2. Find the Volume pane
3. For each visible candle:
   - Calculate bar from 0 to candle.volume
   - Spawn bar sprite with semi-transparent color
   - Color matches candle direction (green/red)

### `render_grid_and_axes`
Renders grid lines, axes labels, and pane separators.

**Frequency**: Every frame when `chart.needs_redraw == true`

**Process**:
1. **Per-pane horizontal grids**:
   - For each pane, draw 8 horizontal lines
   - Add Y-axis labels on right side (price for Price pane, volume for Volume pane)
2. **Pane separators**:
   - Draw semi-transparent gray line between panes
3. **Shared vertical grids**:
   - Draw 10 vertical time lines spanning all panes
   - Add X-axis time labels at bottom of last pane only

### `render_crosshair`
Renders interactive crosshair with dynamic labels.

**Frequency**: Every frame

**Process**:
1. Check if mouse is within any pane
2. **Vertical line**: Spans from top of first pane to bottom of last pane
3. **Per-pane horizontal lines**: Draw horizontal line in each pane that contains the mouse Y position
4. **Value labels**: Show appropriate value (price/volume) for each pane at mouse position
5. **Time label**: Show time at mouse X position at bottom
6. **OHLCV info box**: Display candle data at top-left of Price pane

## Interaction Systems

### `handle_mouse_input`
Handles pan, zoom, and mouse tracking.

**Pan** (Left mouse drag):
- Updates `chart.visible_candle_start`
- Calls `update_pane_bounds()` to refresh all pane Y-axes
- Sets `chart.needs_redraw = true`

**Zoom** (Mouse wheel):
- Updates `chart.visible_candle_count`
- Keeps focus candle at same screen position
- Calls `update_pane_bounds()` to refresh all pane Y-axes
- Sets `chart.needs_redraw = true`

**Mouse tracking**:
- Converts window coordinates (top-left origin, Y down) to world coordinates (center origin, Y up)
- Updates `interaction.mouse_pos`

### `check_lazy_load`
Loads more data from database when scrolling near edges.

**Left edge** (historical data):
- Triggers when `visible_candle_start < 20`
- Loads 100 candles before current range
- Prepends to `chart.candles`
- Adjusts `visible_candle_start` to maintain view

**Right edge** (recent data):
- Triggers when near end of loaded data
- Loads 100 candles after current range
- Appends to `chart.candles`

## Pane Layout System

### `calculate_pane_layouts()`
Distributes screen space among panes from top to bottom.

**Input**:
- `panes: &mut [Pane]`: List of panes to layout
- `total_area: Rect`: Available screen space
- `visible_candle_count: usize`: For cache calculations

**Algorithm**:
```
Starting from top (total_area.max.y):
For each pane:
  1. Calculate height = total_height × pane.height_percent
  2. Assign viewport from current_y to (current_y - height)
  3. Recalculate cached values (candle_width_px, price_scale)
  4. Move current_y down by height
```

**Example**:
```rust
// 70% Price + 30% Volume
let mut panes = vec![
    Pane::new(PaneId::Price, PaneType::Price, 0.7, /* ... */),
    Pane::new(PaneId::Volume, PaneType::Volume, 0.3, /* ... */),
];
calculate_pane_layouts(&mut panes, total_area, 50);
```

### `update_pane_bounds()`
Updates Y-axis bounds for all panes based on visible data.

Called after pan/zoom to ensure each pane shows appropriate value range.

```rust
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
                // TODO: Handle indicators
            }
        }
    }
}
```

## Database Integration

### ChartDatabase
Wrapper around DuckDB connection with Arc<Mutex> for thread safety.

**Methods**:
- `load_candles(ticker_id, timeframe, start_time, end_time) -> Vec<Candle>`
- `get_time_range(ticker_id, timeframe) -> (i64, i64)`

**Schema**:
```sql
CREATE TABLE klines (
    kline_id BIGINT PRIMARY KEY,
    ticker_id INTEGER,
    timeframe VARCHAR,
    candle_time BIGINT,  -- Unix timestamp in ms
    open_price DECIMAL(18,8),
    high_price DECIMAL(18,8),
    low_price DECIMAL(18,8),
    close_price DECIMAL(18,8),
    volume DECIMAL(18,8),
    num_trades INTEGER
);
```

## Configuration Resources

### `ChartGrid`
Grid appearance settings.
```rust
struct ChartGrid {
    show_grid: bool,
    grid_color: Color,
    y_tick_count: usize,  // Horizontal lines per pane (default: 8)
    x_tick_count: usize,  // Vertical time lines (default: 10)
}
```

### `ChartAxes`
Axis label settings.
```rust
struct ChartAxes {
    show_x_labels: bool,
    show_y_labels: bool,
    label_color: Color,
    label_size: f32,
}
```

### `Crosshair`
Crosshair appearance settings.
```rust
struct Crosshair {
    enabled: bool,
    line_color: Color,
    show_ohlcv_box: bool,
    show_price_label: bool,
    show_time_label: bool,
}
```

## Adding New Panes

### Example: Adding an RSI Indicator Pane

**Step 1: Define the indicator**
```rust
struct RSI {
    period: usize,
    values: Vec<f32>,  // Cached RSI values
}

impl RSI {
    fn calculate(&mut self, candles: &[Candle], start: usize, count: usize) {
        // Calculate RSI values for visible range
        // Store in self.values
    }
}
```

**Step 2: Update Chart structure**
```rust
struct Chart {
    // ... existing fields ...

    // Add indicator storage
    rsi: Option<RSI>,
}
```

**Step 3: Add to pane setup**
```rust
// In setup()
let mut panes = vec![
    Pane::new(PaneId::Price, PaneType::Price, 0.6, /* ... */),
    Pane::new(PaneId::Volume, PaneType::Volume, 0.2, /* ... */),
    Pane::new(
        PaneId::Indicator(0),
        PaneType::Indicator { name: "RSI".to_string() },
        0.2,
        /* ... */
    ),
];
```

**Step 4: Create render system**
```rust
fn render_rsi(
    mut commands: Commands,
    chart: Res<Chart>,
    query: Query<Entity, With<RSIElement>>,
) {
    // Find RSI pane
    let rsi_pane = chart.panes.iter()
        .find(|p| matches!(p.pane_type, PaneType::Indicator { name } if name == "RSI"))
        .unwrap();

    // Render RSI line from 0-100 range
    // Use rsi_pane.space.to_world() for coordinate conversion
}
```

**Step 5: Update `update_pane_bounds()`**
```rust
PaneType::Indicator { name } if name == "RSI" => {
    // RSI is always 0-100
    pane.space.visible_price_min = 0.0;
    pane.space.visible_price_max = 100.0;
    pane.space.recalculate_cache(chart.visible_candle_count);
}
```

**Step 6: Add to system schedule**
```rust
.add_systems(Update, render_rsi)
```

## Performance Considerations

### Despawn Pattern
All rendering systems despawn previous entities before creating new ones:
```rust
for entity in query.iter() {
    commands.entity(entity).despawn();
}
```

This prevents entity accumulation but generates B0003 warnings (harmless).

### Lazy Loading
- Only loads visible data + buffer (20 candles on each side)
- Loads in chunks of 100 candles
- Prevents memory issues with large datasets

### Cached Calculations
ChartSpace caches frequently used values:
- `candle_width_px`: Width of each candle in pixels
- `price_scale`: Pixels per unit of value

Recalculated only when:
- Viewport changes (pane resize)
- Visible candle count changes (zoom)

## Common Patterns

### Finding a Specific Pane
```rust
let price_pane = chart.panes.iter()
    .find(|p| matches!(p.id, PaneId::Price))
    .unwrap();
```

### Iterating Through Visible Candles
```rust
let start = chart.visible_candle_start;
let end = (start + chart.visible_candle_count).min(chart.candles.len());

for i in start..end {
    let candle = &chart.candles[i];
    // Process candle...
}
```

### Converting Coordinates
```rust
// Data to screen
let screen_pos = pane.space.to_world(
    candle_index,
    value,
    chart.visible_candle_start,
    chart.visible_candle_count
);

// Screen to data
let (candle_index, value) = pane.space.from_world(
    mouse_pos,
    chart.visible_candle_start,
    chart.visible_candle_count
);
```

## Troubleshooting

### Grid/Axes Not Showing
- Check `ChartGrid::show_grid` is true
- Check `ChartAxes::show_x_labels` and `show_y_labels` are true
- Verify `chart.needs_redraw` is being set to true

### Crosshair Not Working
- Check `Crosshair::enabled` is true
- Verify mouse position conversion in `handle_mouse_input`
- Check pane viewport contains mouse position

### Y-Axis Labels Wrong
- Ensure each pane uses correct `fit_*_bounds()` method:
  - Price pane: `fit_price_bounds()`
  - Volume pane: `fit_volume_bounds()`
- Check `update_pane_bounds()` is called after pan/zoom

### Panes Not Separated Visually
- Separator line is rendered in `render_grid_and_axes()`
- Check Z-order (separator uses Z=0.5)
- Verify separator color has sufficient alpha

## Future Enhancements

### Planned Features
- [ ] Additional indicators (RSI, MACD, Stochastic)
- [ ] Overlays on price chart (Moving Averages, Bollinger Bands)
- [ ] Drawing tools (trend lines, rectangles)
- [ ] Multiple symbol support
- [ ] Real-time WebSocket updates
- [ ] Configurable pane heights (draggable splitters)
- [ ] Save/load chart layouts
- [ ] Custom color schemes

### Architecture Notes
The current multi-pane system is designed to support:
- Unlimited number of panes
- Each pane can have independent data source
- Each pane can have independent rendering logic
- All panes share synchronized X-axis (time)

## Version History

### Current (Multi-Pane Implementation)
- Multi-pane layout with Price (70%) + Volume (30%)
- Per-pane grids and Y-axis labels
- Spanning crosshair across all panes
- Visual pane separators
- Synchronized pan/zoom

### Previous Commits
- Fix crosshair horizontal line inversion
- Implement crosshair and OHLCV info panel
- Fix grid axes: Make labels visible on screen
- Implement Phase 2: Grid and Axes System
- Implement Phase 1: Basic candlestick chart with pan/zoom
