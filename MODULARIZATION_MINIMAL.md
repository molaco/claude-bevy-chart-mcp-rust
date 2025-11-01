# Minimal Modularization Plan

## Goal
Split the 1283-line `main.rs` into **3 focused modules** with clear responsibilities.

---

## Proposed Structure

```
charts/src/
├── main.rs           (~150 lines) - App setup & plugin registration
├── types.rs          (~400 lines) - All data structures & configs
├── rendering.rs      (~500 lines) - All rendering systems
└── interaction.rs    (~250 lines) - Mouse input & lazy loading
```

**Total**: ~1300 lines (similar to current, just organized)

---

## Module Breakdown

### 1. `types.rs` - Data Structures & Configuration
**Lines**: ~400

**Contents**:
- `struct Candle` (main.rs:6-20)
- `struct ChartSpace` + impl (main.rs:22-149)
- `enum PaneId` (main.rs:152-158)
- `enum PaneType` (main.rs:161-166)
- `struct Pane` + impl (main.rs:169-176)
- `struct MouseInteraction` (main.rs:179-185)
- `fn calculate_pane_layouts()` (main.rs:188-217)
- `struct Chart` (main.rs:220-242)
- `struct ChartGrid` (main.rs:245-251)
- `struct ChartAxes` (main.rs:254-262)
- `struct Crosshair` (main.rs:265-273)
- `struct ChartElement` (main.rs:276-283)
- `struct ChartDatabase` + impl (main.rs:286-413)
- `fn update_pane_bounds()` (main.rs:1151-1174)

**Why together**: All type definitions and core logic that other modules depend on.

---

### 2. `rendering.rs` - All Rendering Systems
**Lines**: ~500

**Contents**:
- `fn render_candlesticks()` (main.rs:512-622)
- `struct VolumeBarElement` (main.rs:625-627)
- `fn render_volume_bars()` (main.rs:630-710)
- `struct GridElement` (main.rs:707-709)
- `fn render_grid_and_axes()` (main.rs:712-872)
- `struct CrosshairElement` (main.rs:874-876)
- `fn render_crosshair()` (main.rs:874-1052)

**Imports needed**:
```rust
use bevy::prelude::*;
use chrono::{DateTime, Utc};
use crate::types::*;
```

**Why together**: All systems that draw to the screen. Clear visual/rendering boundary.

---

### 3. `interaction.rs` - User Input & Data Loading
**Lines**: ~250

**Contents**:
- `fn handle_mouse_input()` (main.rs:1054-1149)
- `fn check_lazy_load()` (main.rs:1176-1258)

**Imports needed**:
```rust
use bevy::prelude::*;
use crate::types::*;
```

**Why together**: Both deal with user interaction and data state changes.

---

### 4. `main.rs` - Application Entry
**Lines**: ~150

**Contents**:
- `fn main()` (main.rs:1261-1283)
- `fn setup()` (main.rs:416-510)

**New structure**:
```rust
mod types;
mod rendering;
mod interaction;

use bevy::prelude::*;
use types::*;
use rendering::*;
use interaction::*;

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
        .insert_resource(ChartGrid::default())
        .insert_resource(ChartAxes::default())
        .insert_resource(Crosshair::default())
        .insert_resource(MouseInteraction::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (
            render_candlesticks,
            render_volume_bars,
            render_grid_and_axes,
            render_crosshair,
            handle_mouse_input,
            check_lazy_load,
        ))
        .run();
}

fn setup(/* ... */) {
    // Same as current setup() function
}
```

---

## Migration Steps

### Step 1: Create `types.rs` (20 min)
```bash
# 1. Create new file
touch src/types.rs

# 2. Copy all structs, enums, and core functions
# 3. Add module header:
use bevy::prelude::*;
use duckdb::{Connection, Result};
use std::sync::{Arc, Mutex};

# 4. Make all items public: pub struct, pub fn, pub enum
# 5. Test: cargo check
```

### Step 2: Create `rendering.rs` (15 min)
```bash
# 1. Create new file
touch src/rendering.rs

# 2. Copy all render_* functions and marker structs
# 3. Add imports:
use bevy::prelude::*;
use chrono::{DateTime, Utc};
use crate::types::*;

# 4. Make functions public: pub fn
# 5. Test: cargo check
```

### Step 3: Create `interaction.rs` (10 min)
```bash
# 1. Create new file
touch src/interaction.rs

# 2. Copy handle_mouse_input() and check_lazy_load()
# 3. Add imports:
use bevy::prelude::*;
use crate::types::*;

# 4. Make functions public: pub fn
# 5. Test: cargo check
```

### Step 4: Update `main.rs` (10 min)
```bash
# 1. Add module declarations at top:
mod types;
mod rendering;
mod interaction;

# 2. Add use statements:
use types::*;
use rendering::*;
use interaction::*;

# 3. Remove all copied code (keep main() and setup())
# 4. Test: cargo check
# 5. Run: cargo run
```

### Step 5: Verify (5 min)
- [ ] Application launches
- [ ] Pan works (left mouse drag)
- [ ] Zoom works (mouse wheel)
- [ ] Crosshair works (mouse move)
- [ ] Lazy loading works (scroll to edges)
- [ ] No compiler warnings

**Total time**: ~60 minutes

---

## File Size Comparison

| File | Before | After | Change |
|------|--------|-------|--------|
| main.rs | 1283 lines | 150 lines | -88% |
| types.rs | - | 400 lines | New |
| rendering.rs | - | 500 lines | New |
| interaction.rs | - | 250 lines | New |
| **Largest file** | 1283 | 500 | -61% |

---

## Benefits

✅ **Simple**: Only 3 new files, flat structure
✅ **Clear boundaries**: Types vs Rendering vs Interaction
✅ **Easy to navigate**: Know where to look for each concern
✅ **Low risk**: Minimal code changes, just moving
✅ **Fast migration**: ~1 hour total

---

## When to Add More Modules

Consider further splitting **only if**:
- `rendering.rs` grows beyond 800 lines → split by system (candlesticks, volume, grid, crosshair)
- `types.rs` grows beyond 600 lines → split into types + database
- Adding new major features (indicators, themes, export)

**Philosophy**: Start simple, split when needed.

---

## Alternative: Even More Minimal (2 modules)

If you want even simpler:

```
src/
├── main.rs      (~150 lines) - App setup
├── types.rs     (~400 lines) - All data structures
└── systems.rs   (~750 lines) - All Bevy systems (rendering + interaction)
```

**Pros**: Simplest possible split
**Cons**: `systems.rs` still quite large (750 lines)

---

## Recommendation

**Use the 3-module approach** (types, rendering, interaction):
- Clear separation of concerns
- Each file manageable size (250-500 lines)
- Natural boundaries that match mental model
- Easy to explain to new contributors
- Room to grow without immediate need to refactor
