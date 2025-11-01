# Modularization Plan

## Executive Summary

Transform the monolithic 1283-line `main.rs` into a well-organized module structure that:
- Separates concerns by domain (data, rendering, interaction)
- Makes code easier to test and extend
- Follows Rust best practices
- Enables parallel development on different features

**Estimated effort**: 4-6 hours
**Risk level**: Low (mostly moving code, minimal logic changes)
**Benefits**:
- 60% reduction in file size per module
- Easier to add new indicators/panes
- Better compile times (incremental)
- Clearer dependency graph

---

## Proposed Directory Structure

```
charts/
├── Cargo.toml
└── src/
    ├── main.rs                    (100-150 lines)
    ├── lib.rs                     (50 lines)
    │
    ├── core/
    │   ├── mod.rs                 (30 lines)
    │   ├── candle.rs              (40 lines)
    │   ├── chart.rs               (120 lines)
    │   ├── pane.rs                (100 lines)
    │   └── space.rs               (150 lines)
    │
    ├── config/
    │   ├── mod.rs                 (30 lines)
    │   ├── grid.rs                (30 lines)
    │   ├── axes.rs                (30 lines)
    │   └── crosshair.rs           (40 lines)
    │
    ├── components/
    │   ├── mod.rs                 (60 lines)
    │   └── markers.rs             (50 lines)
    │
    ├── database/
    │   ├── mod.rs                 (20 lines)
    │   └── duckdb.rs              (100 lines)
    │
    ├── layout/
    │   ├── mod.rs                 (30 lines)
    │   ├── calculator.rs          (80 lines)
    │   └── bounds.rs              (60 lines)
    │
    ├── rendering/
    │   ├── mod.rs                 (60 lines)
    │   ├── candlesticks.rs        (150 lines)
    │   ├── volume.rs              (120 lines)
    │   ├── grid.rs                (200 lines)
    │   └── crosshair.rs           (220 lines)
    │
    └── interaction/
        ├── mod.rs                 (30 lines)
        ├── mouse.rs               (120 lines)
        └── lazy_load.rs           (80 lines)
```

**Total**: ~1800 lines (includes module declarations, better spacing)
**Largest file**: ~220 lines (crosshair.rs)

---

## Module Breakdown

### 1. `src/lib.rs` - Library Root
**Purpose**: Re-export public API for use by main.rs and tests

**Contents**:
```rust
pub mod core;
pub mod config;
pub mod components;
pub mod database;
pub mod layout;
pub mod rendering;
pub mod interaction;

// Re-export commonly used items
pub use core::{Candle, Chart, Pane, PaneId, PaneType, ChartSpace};
pub use config::{ChartGrid, ChartAxes, Crosshair};
pub use database::ChartDatabase;
```

**Migration**: Create new file
**Dependencies**: None (root)

---

### 2. `src/core/` - Core Data Structures

#### `core/candle.rs`
**Purpose**: OHLCV data structure

**Current location**: main.rs:6-20
**Contents**:
- `struct Candle`
- `impl Candle` (if any utility methods added)

**Dependencies**: None

#### `core/space.rs`
**Purpose**: Coordinate transformation system

**Current location**: main.rs:22-149
**Contents**:
- `struct ChartSpace`
- `impl ChartSpace`
  - `new()`
  - `to_world()`
  - `from_world()`
  - `fit_price_bounds()`
  - `fit_volume_bounds()`
  - `recalculate_cache()`

**Dependencies**:
- `core::candle::Candle`
- `bevy::prelude::*`

#### `core/pane.rs`
**Purpose**: Individual pane configuration

**Current location**: main.rs:152-176
**Contents**:
- `enum PaneId`
- `enum PaneType`
- `struct Pane`
- `impl Pane::new()`

**Dependencies**:
- `core::space::ChartSpace`
- `bevy::prelude::*`

#### `core/chart.rs`
**Purpose**: Main chart resource

**Current location**: main.rs:220-242
**Contents**:
- `struct Chart`
- `impl Chart` (utility methods like finding panes)

**Example additions**:
```rust
impl Chart {
    pub fn find_pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    pub fn find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    pub fn visible_candles(&self) -> &[Candle] {
        let start = self.visible_candle_start;
        let end = (start + self.visible_candle_count).min(self.candles.len());
        &self.candles[start..end]
    }
}
```

**Dependencies**:
- `core::candle::Candle`
- `core::pane::Pane`
- `core::space::ChartSpace`

#### `core/mod.rs`
**Purpose**: Module root, re-exports

**Contents**:
```rust
mod candle;
mod space;
mod pane;
mod chart;

pub use candle::Candle;
pub use space::ChartSpace;
pub use pane::{Pane, PaneId, PaneType};
pub use chart::Chart;
```

---

### 3. `src/config/` - Configuration Resources

#### `config/grid.rs`
**Current location**: main.rs:245-251
**Contents**: `struct ChartGrid`, `impl Default`

#### `config/axes.rs`
**Current location**: main.rs:254-262
**Contents**: `struct ChartAxes`, `impl Default`

#### `config/crosshair.rs`
**Current location**: main.rs:265-273
**Contents**: `struct Crosshair`, `impl Default`

#### `config/mod.rs`
**Contents**:
```rust
mod grid;
mod axes;
mod crosshair;

pub use grid::ChartGrid;
pub use axes::ChartAxes;
pub use crosshair::Crosshair;
```

**Dependencies**: `bevy::prelude::*`

---

### 4. `src/components/` - ECS Components

#### `components/markers.rs`
**Purpose**: Marker components for entity queries

**Current location**: main.rs:276-283, 625-627, 707-709
**Contents**:
- `struct ChartElement` (marker)
- `struct VolumeBarElement` (marker)
- `struct GridElement` (marker)
- `struct CrosshairElement` (marker)

**Dependencies**: `bevy::prelude::*`

#### `components/mod.rs`
**Contents**:
```rust
mod markers;

pub use markers::{ChartElement, VolumeBarElement, GridElement, CrosshairElement};
```

---

### 5. `src/database/` - Database Layer

#### `database/duckdb.rs`
**Purpose**: DuckDB integration

**Current location**: main.rs:286-413
**Contents**:
- `struct ChartDatabase`
- `impl ChartDatabase`
  - `new()`
  - `load_candles()`
  - `get_time_range()`

**Dependencies**:
- `core::candle::Candle`
- `duckdb::*`
- `std::sync::{Arc, Mutex}`

#### `database/mod.rs`
**Contents**:
```rust
mod duckdb;

pub use duckdb::ChartDatabase;
```

---

### 6. `src/layout/` - Layout Management

#### `layout/calculator.rs`
**Purpose**: Pane layout calculation

**Current location**: main.rs:188-217
**Contents**:
- `pub fn calculate_pane_layouts(panes: &mut [Pane], total_area: Rect, visible_candle_count: usize)`

**Dependencies**:
- `core::pane::Pane`
- `bevy::prelude::*`

#### `layout/bounds.rs`
**Purpose**: Y-axis bounds management

**Current location**: main.rs:1151-1174
**Contents**:
- `pub fn update_pane_bounds(chart: &mut Chart)`

**Dependencies**:
- `core::chart::Chart`
- `core::pane::PaneType`

#### `layout/mod.rs`
**Contents**:
```rust
mod calculator;
mod bounds;

pub use calculator::calculate_pane_layouts;
pub use bounds::update_pane_bounds;
```

---

### 7. `src/rendering/` - Rendering Systems

#### `rendering/candlesticks.rs`
**Purpose**: Candlestick rendering system

**Current location**: main.rs:512-622
**Contents**:
- `pub fn render_candlesticks(commands, chart, query)`

**Dependencies**:
- `core::{Chart, PaneId}`
- `components::ChartElement`
- `bevy::prelude::*`

#### `rendering/volume.rs`
**Purpose**: Volume bar rendering system

**Current location**: main.rs:630-710
**Contents**:
- `pub fn render_volume_bars(commands, chart, query)`

**Dependencies**:
- `core::{Chart, PaneId}`
- `components::VolumeBarElement`
- `bevy::prelude::*`

#### `rendering/grid.rs`
**Purpose**: Grid and axes rendering system

**Current location**: main.rs:712-872
**Contents**:
- `pub fn render_grid_and_axes(commands, chart, grid, axes, query)`
- Helper functions:
  - `format_price(price: f32) -> String`
  - `format_volume(volume: f32) -> String`
  - `format_timestamp(timestamp: i64) -> String`

**Dependencies**:
- `core::Chart`
- `config::{ChartGrid, ChartAxes}`
- `components::GridElement`
- `bevy::prelude::*`
- `chrono::*`

#### `rendering/crosshair.rs`
**Purpose**: Interactive crosshair rendering

**Current location**: main.rs:874-1052
**Contents**:
- `pub fn render_crosshair(commands, chart, crosshair, interaction, query)`
- Helper functions:
  - `format_ohlcv_info(candle: &Candle) -> String`

**Dependencies**:
- `core::{Chart, Candle, PaneId}`
- `config::Crosshair`
- `components::CrosshairElement`
- `interaction::MouseInteraction`
- `bevy::prelude::*`

#### `rendering/mod.rs`
**Contents**:
```rust
mod candlesticks;
mod volume;
mod grid;
mod crosshair;

pub use candlesticks::render_candlesticks;
pub use volume::render_volume_bars;
pub use grid::render_grid_and_axes;
pub use crosshair::render_crosshair;

// Plugin for easy registration
use bevy::prelude::*;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            render_candlesticks,
            render_volume_bars,
            render_grid_and_axes,
            render_crosshair,
        ));
    }
}
```

---

### 8. `src/interaction/` - Interaction Systems

#### `interaction/mouse.rs`
**Purpose**: Mouse input handling

**Current location**: main.rs:179-185, 1054-1149
**Contents**:
- `struct MouseInteraction` (resource)
- `pub fn handle_mouse_input(buttons, windows, camera_query, chart, interaction, db, grid, axes)`

**Dependencies**:
- `core::Chart`
- `config::{ChartGrid, ChartAxes}`
- `database::ChartDatabase`
- `layout::update_pane_bounds`
- `bevy::prelude::*`

#### `interaction/lazy_load.rs`
**Purpose**: Lazy data loading system

**Current location**: main.rs:1176-1258
**Contents**:
- `pub fn check_lazy_load(chart, db)`

**Dependencies**:
- `core::Chart`
- `database::ChartDatabase`
- `layout::update_pane_bounds`
- `bevy::prelude::*`

#### `interaction/mod.rs`
**Contents**:
```rust
mod mouse;
mod lazy_load;

pub use mouse::{MouseInteraction, handle_mouse_input};
pub use lazy_load::check_lazy_load;

// Plugin for easy registration
use bevy::prelude::*;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MouseInteraction::default())
           .add_systems(Update, (
               handle_mouse_input,
               check_lazy_load,
           ));
    }
}
```

---

### 9. `src/main.rs` - Application Entry Point

**New structure** (~100-150 lines):
```rust
use bevy::prelude::*;
use charts::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Chart".to_string(),
                resolution: (1600.0, 900.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(rendering::RenderingPlugin)
        .add_plugins(interaction::InteractionPlugin)
        .insert_resource(ChartGrid::default())
        .insert_resource(ChartAxes::default())
        .insert_resource(Crosshair::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    windows: Query<&Window>,
) {
    // Camera setup
    commands.spawn(Camera2dBundle::default());

    // Database connection
    let db_path = "path/to/klines.db";
    let db = ChartDatabase::new(db_path).expect("Failed to connect to database");

    // Load initial data
    let ticker_id = 1;
    let timeframe = "1h".to_string();
    let (min_time, max_time) = db.get_time_range(ticker_id, &timeframe).unwrap();
    let end_time = max_time;
    let start_time = end_time - (200 * 3600 * 1000);

    let candles = db.load_candles(ticker_id, &timeframe, start_time, end_time);

    let visible_candle_count = 50;
    let visible_candle_start = if candles.len() > visible_candle_count {
        candles.len() - visible_candle_count
    } else {
        0
    };

    // Chart viewport
    let window = windows.single();
    let window_width = window.width();
    let window_height = window.height();

    let total_area = Rect::from_center_size(
        Vec2::new(-40.0, 10.0),
        Vec2::new(window_width - 200.0, window_height - 120.0),
    );

    // Create panes
    let mut panes = vec![
        Pane::new(PaneId::Price, PaneType::Price, 0.7, Rect::default(), visible_candle_count),
        Pane::new(PaneId::Volume, PaneType::Volume, 0.3, Rect::default(), visible_candle_count),
    ];

    // Calculate layouts
    layout::calculate_pane_layouts(&mut panes, total_area, visible_candle_count);

    // Fit Y-axis bounds
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

    // Insert Chart resource
    commands.insert_resource(Chart {
        ticker_id,
        timeframe,
        candles,
        candle_offset: 0,
        visible_candle_start,
        visible_candle_count,
        panes,
        space: ChartSpace::default(), // Deprecated
        needs_redraw: true,
        loading: false,
    });

    // Insert database resource
    commands.insert_resource(db);
}
```

---

## Migration Strategy

### Phase 1: Preparation (30 min)
1. **Create branch**: `git checkout -b refactor/modularization`
2. **Create lib.rs**: Empty file with module declarations
3. **Create directory structure**: All folders and `mod.rs` files
4. **Run tests**: Ensure baseline passes

### Phase 2: Core Modules (1 hour)
**Order**: core → config → components → database

For each module:
1. Copy code from main.rs
2. Add imports
3. Update main.rs to use `use charts::module::Type`
4. Run `cargo check`
5. Fix errors
6. Commit

**Example**:
```bash
git commit -m "refactor: Extract core::candle module"
git commit -m "refactor: Extract core::space module"
git commit -m "refactor: Extract core::pane module"
git commit -m "refactor: Extract core::chart module"
```

### Phase 3: Layout & Rendering (1.5 hours)
**Order**: layout → rendering (candlesticks, volume, grid, crosshair)

Rendering modules are independent, can be done in parallel if needed.

### Phase 4: Interaction (45 min)
**Order**: interaction/mouse → interaction/lazy_load

### Phase 5: Cleanup (30 min)
1. Remove old code from main.rs
2. Add module-level documentation
3. Run full test suite
4. Check for unused imports
5. Format code: `cargo fmt`
6. Run clippy: `cargo clippy`

### Phase 6: Validation (30 min)
1. Run application: `cargo run`
2. Test all interactions:
   - Pan (left mouse drag)
   - Zoom (mouse wheel)
   - Crosshair (mouse move)
   - Lazy loading (scroll to edges)
3. Check for warnings
4. Profile build times (should be slightly better)

---

## Testing Strategy

### Unit Tests
Add to each module:

**Example** (`core/space.rs`):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_world_basic() {
        let space = ChartSpace {
            visible_price_min: 0.0,
            visible_price_max: 100.0,
            viewport: Rect::from_corners(Vec2::ZERO, Vec2::new(100.0, 100.0)),
            ..default()
        };

        let pos = space.to_world(25, 50.0, 0, 100);
        assert_eq!(pos.x, 25.0);
        assert_eq!(pos.y, 50.0);
    }

    #[test]
    fn test_from_world_inverse() {
        let space = ChartSpace { /* ... */ };
        let original_pos = Vec2::new(50.0, 75.0);

        let (idx, price) = space.from_world(original_pos, 0, 100);
        let reconstructed = space.to_world(idx, price, 0, 100);

        assert!((reconstructed.x - original_pos.x).abs() < 1.0);
        assert!((reconstructed.y - original_pos.y).abs() < 1.0);
    }
}
```

### Integration Tests
Create `charts/tests/integration_test.rs`:
```rust
use charts::*;

#[test]
fn test_chart_creation() {
    let candles = vec![/* test data */];
    let mut panes = vec![/* test panes */];

    layout::calculate_pane_layouts(
        &mut panes,
        Rect::from_corners(Vec2::ZERO, Vec2::new(800.0, 600.0)),
        50
    );

    assert!(panes[0].space.viewport.height() > 0.0);
    assert!(panes[1].space.viewport.height() > 0.0);
}
```

---

## Benefits Analysis

### Code Organization
| Aspect | Before | After | Improvement |
|--------|--------|-------|-------------|
| Largest file | 1283 lines | ~220 lines | 83% reduction |
| Average file size | 1283 lines | ~90 lines | 93% reduction |
| Files to modify for new indicator | 1 | 2-3 | Clearer scope |
| Module dependencies | N/A | Explicit | Better design |

### Development Workflow
- **Parallel development**: Multiple developers can work on different rendering systems without conflicts
- **Testing**: Can test layout logic without rendering dependencies
- **Compilation**: Incremental builds only recompile changed modules
- **Navigation**: Jump to module by domain (rendering, interaction) instead of line number

### Extensibility Examples

**Adding a new indicator pane**:
```rust
// 1. Define in core/pane.rs
PaneType::Indicator { name: "RSI".to_string() }

// 2. Create rendering/rsi.rs
pub fn render_rsi(/* ... */) { /* ... */ }

// 3. Register in rendering/mod.rs
pub use rsi::render_rsi;

// 4. Add to RenderingPlugin
app.add_systems(Update, render_rsi);
```

**Before**: Need to understand entire main.rs (1283 lines)
**After**: Only need to understand rendering module pattern (~150 lines of reference)

---

## Risks and Mitigations

### Risk 1: Import Cycles
**Symptom**: `cargo check` fails with "cyclic dependency"
**Mitigation**: Follow dependency order (core → config → components → other)
**Solution**: Use trait objects or restructure if cycle detected

### Risk 2: Bevy System Ordering
**Symptom**: Rendering happens before data updates
**Mitigation**: Keep system ordering explicit in main.rs
**Validation**: Test all interactions after migration

### Risk 3: Resource Lifetime Issues
**Symptom**: Mutable borrow conflicts
**Mitigation**: Keep resource access patterns unchanged during migration
**Solution**: Use `Res`/`ResMut` correctly, avoid unnecessary borrowing

### Risk 4: Performance Regression
**Symptom**: FPS drops after modularization
**Likelihood**: Very low (code logic unchanged)
**Validation**: Profile before/after with `cargo flamegraph`

---

## Success Criteria

✅ **All tests pass**
✅ **Application runs without errors**
✅ **No clippy warnings**
✅ **All interactions work (pan, zoom, crosshair)**
✅ **Lazy loading functions correctly**
✅ **Build time ≤ current build time + 5%**
✅ **Code coverage ≥ 50%** (stretch goal)

---

## Future Extensions

Once modularized, these become easier:

### 1. Indicator System
```
src/indicators/
├── mod.rs
├── rsi.rs
├── macd.rs
├── bollinger.rs
└── moving_average.rs
```

### 2. Plugin Architecture
```rust
pub trait IndicatorPlugin {
    fn name(&self) -> &str;
    fn calculate(&self, candles: &[Candle]) -> Vec<f32>;
    fn render(&self, commands: &mut Commands, data: &[f32]);
}
```

### 3. Theme System
```
src/themes/
├── mod.rs
├── dark.rs
├── light.rs
└── tradingview.rs
```

### 4. Export System
```
src/export/
├── mod.rs
├── png.rs
├── svg.rs
└── csv.rs
```

---

## Conclusion

This modularization plan provides:
- **Clear roadmap**: Step-by-step migration
- **Low risk**: Incremental changes with validation
- **High value**: Better maintainability and extensibility
- **Future-proof**: Supports planned features (indicators, themes, plugins)

**Recommendation**: Execute this plan before implementing OPTION_C_POLISH.md features. A clean module structure will make those features easier to implement and maintain.

**Estimated timeline**:
- **Solo developer**: 1 day (6 hours focused work)
- **With tests**: 1.5 days (8-10 hours)
- **With documentation**: 2 days (12 hours)
