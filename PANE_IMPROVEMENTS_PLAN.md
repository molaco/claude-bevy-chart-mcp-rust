# Pane Improvements Plan

## Overview

This document outlines the plan to improve pane separation, add resize functionality, and implement pane toggling in the Bevy chart application.

---

## Problem 1: Visual Separation

### Current Issues
- Separator line is only 2px and semi-transparent (0.5 alpha)
- No padding between panes - grids touch the separator
- Y-axis labels from different panes are at same X position, causing visual confusion
- No clear indication which pane you're looking at

### Proposed Solution

#### 1. Thicker, More Visible Separator
- **Current**: 2px line with `Color::srgba(0.5, 0.5, 0.5, 0.5)`
- **Proposed**: 6-8px line with more opaque color
- **Location**: `rendering.rs:800-812` (render_grid_and_axes)

#### 2. Add Padding Between Panes
- Add 10-15px gap on each side of separator
- Modify `calculate_pane_layouts()` in `types.rs` to leave space for separator
- Reduces viewport height for each pane slightly but improves visual clarity

#### 3. Different Y-Axis Label Treatment
- Price pane: labels on the RIGHT (current position)
- Volume pane: labels on the RIGHT but with different color or add pane identifier
- Alternative: Add small "PRICE" / "VOLUME" prefix to labels

#### 4. Add Pane Title Headers
- Display "PRICE" / "VOLUME" at top-left of each pane
- Font size: 18-20px
- Color: Slightly dimmed (0.6, 0.6, 0.6)
- Position: 20px from left edge, 10px below top of pane

#### 5. Optional: Subtle Background Shading
- Price pane: Very slight blue tint (`Color::srgba(0.0, 0.0, 0.05, 1.0)`)
- Volume pane: Very slight different tint
- Helps distinguish panes without being distracting

---

## Problem 2: Pane Resize (Draggable Splitters)

### Implementation Plan

#### Step 1: Data Structure Changes

**File**: `types.rs`

**Add new component:**
```rust
/// Component to identify and track pane separators
#[derive(Component)]
pub struct Separator {
    pub pane_index: usize,  // Index of pane above this separator
    pub y_position: f32,    // World Y coordinate of separator
}
```

**Modify `InteractionState`:**
```rust
#[derive(Resource, Default)]
pub struct InteractionState {
    pub mouse_pos: Vec2,
    pub dragging: bool,
    pub drag_start_pos: Vec2,

    // NEW: Resize state
    pub resizing_pane: Option<usize>,  // Index of pane being resized
    pub resize_start_y: f32,           // Where resize started
    pub hover_separator: Option<usize>, // Which separator is being hovered
}
```

#### Step 2: Separator Tracking

**File**: `rendering.rs` - `render_grid_and_axes()`

**Modify separator spawning (around line 800):**
```rust
// ========== PANE SEPARATORS ==========
for i in 0..chart.panes.len() - 1 {
    let pane_bottom = chart.panes[i].space.viewport.min.y;
    let separator_y = pane_bottom;
    let line_center = Vec2::new((chart_left + chart_right) / 2.0, separator_y);
    let line_width = chart_right - chart_left;

    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::srgba(0.7, 0.7, 0.7, 0.8), // More visible
                custom_size: Some(Vec2::new(line_width, 6.0)), // Thicker
                ..default()
            },
            transform: Transform::from_translation(line_center.extend(0.5)),
            ..default()
        },
        GridElement,
        Separator {
            pane_index: i,
            y_position: separator_y,
        },
    ));
}
```

#### Step 3: Mouse Interaction

**File**: `interaction.rs`

**Add new system: `handle_pane_resize()`**
```rust
pub fn handle_pane_resize(
    mut chart: ResMut<Chart>,
    mut interaction: ResMut<InteractionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    separator_query: Query<&Separator>,
    mut windows: Query<&mut Window>,
) {
    const HOVER_THRESHOLD: f32 = 5.0; // Pixels

    // Check if hovering over a separator
    interaction.hover_separator = None;
    for separator in separator_query.iter() {
        let distance = (interaction.mouse_pos.y - separator.y_position).abs();
        if distance < HOVER_THRESHOLD {
            interaction.hover_separator = Some(separator.pane_index);

            // Change cursor to resize cursor
            if let Ok(mut window) = windows.get_single_mut() {
                window.cursor.icon = CursorIcon::NsResize;
            }
            break;
        }
    }

    // Reset cursor if not hovering
    if interaction.hover_separator.is_none() && interaction.resizing_pane.is_none() {
        if let Ok(mut window) = windows.get_single_mut() {
            window.cursor.icon = CursorIcon::Default;
        }
    }

    // Start resize on left click while hovering separator
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some(pane_idx) = interaction.hover_separator {
            interaction.resizing_pane = Some(pane_idx);
            interaction.resize_start_y = interaction.mouse_pos.y;
        }
    }

    // Stop resize on release
    if mouse_button.just_released(MouseButton::Left) {
        interaction.resizing_pane = None;
        if let Ok(mut window) = windows.get_single_mut() {
            window.cursor.icon = CursorIcon::Default;
        }
    }

    // Handle resize drag
    if let Some(pane_idx) = interaction.resizing_pane {
        let delta_y = interaction.mouse_pos.y - interaction.resize_start_y;

        // Calculate new height percentages
        // pane_idx is the pane ABOVE the separator
        // pane_idx + 1 is the pane BELOW the separator

        if pane_idx + 1 < chart.panes.len() {
            let total_height = /* calculate total height from viewports */;
            let delta_percent = delta_y / total_height;

            // Update height percentages
            let old_height_above = chart.panes[pane_idx].height_percent;
            let old_height_below = chart.panes[pane_idx + 1].height_percent;

            let new_height_above = (old_height_above - delta_percent).clamp(0.1, 0.9);
            let new_height_below = (old_height_below + delta_percent).clamp(0.1, 0.9);

            // Ensure we're not going below minimum
            if new_height_above >= 0.1 && new_height_below >= 0.1 {
                chart.panes[pane_idx].height_percent = new_height_above;
                chart.panes[pane_idx + 1].height_percent = new_height_below;

                // Recalculate layouts
                calculate_pane_layouts(&mut chart.panes, /* total_area */, chart.visible_candle_count);
                update_pane_bounds(&mut chart);

                chart.needs_redraw = true;
                interaction.resize_start_y = interaction.mouse_pos.y;
            }
        }
    }
}
```

#### Step 4: Constraints

**Minimum/Maximum Heights:**
- Minimum pane height: 10% (78px for 780px chart height)
- Maximum pane height: 90%
- Prevent resize if would violate constraints

**Smooth Dragging:**
- Update on every mouse move during drag
- Clamp values to prevent invalid states
- Visual feedback via cursor change

#### Step 5: System Registration

**File**: `main.rs`

Add to Update systems:
```rust
.add_systems(Update, handle_pane_resize)
```

---

## Problem 3: Pane Toggle (Show/Hide)

### Implementation Plan

#### Step 1: Data Structure Changes

**File**: `types.rs`

**Modify `Pane` struct:**
```rust
#[derive(Clone)]
pub struct Pane {
    pub id: PaneId,
    pub pane_type: PaneType,
    pub height_percent: f32,
    pub space: ChartSpace,
    pub visible: bool,  // NEW: Track visibility
}

impl Pane {
    pub fn new(id: PaneId, pane_type: PaneType, height_percent: f32, viewport: Rect, visible_candle_count: usize) -> Self {
        Self {
            id,
            pane_type,
            height_percent,
            space: ChartSpace::new(viewport, visible_candle_count),
            visible: true,  // Default to visible
        }
    }
}
```

**Add helper method to `Chart`:**
```rust
impl Chart {
    pub fn toggle_pane(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == pane_id) {
            pane.visible = !pane.visible;
        }

        // Prevent hiding all panes
        let visible_count = self.panes.iter().filter(|p| p.visible).count();
        if visible_count == 0 {
            // Re-show the pane we just tried to hide
            if let Some(pane) = self.panes.iter_mut().find(|p| p.id == pane_id) {
                pane.visible = true;
            }
        }
    }

    pub fn get_visible_panes(&self) -> Vec<&Pane> {
        self.panes.iter().filter(|p| p.visible).collect()
    }

    pub fn get_visible_panes_mut(&mut self) -> Vec<&mut Pane> {
        self.panes.iter_mut().filter(|p| p.visible).collect()
    }
}
```

#### Step 2: Keyboard Controls

**File**: `interaction.rs`

**Add new system:**
```rust
use bevy::input::keyboard::KeyCode;

pub fn handle_keyboard_input(
    mut chart: ResMut<Chart>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // Toggle Volume pane with 'V' key
    if keyboard.just_pressed(KeyCode::KeyV) {
        chart.toggle_pane(PaneId::Volume);
        chart.needs_redraw = true;
        println!("Toggled Volume pane");
    }

    // Toggle Price pane with 'P' key (but prevent if it's the only pane)
    if keyboard.just_pressed(KeyCode::KeyP) {
        let visible_count = chart.panes.iter().filter(|p| p.visible).count();
        if visible_count > 1 {
            chart.toggle_pane(PaneId::Price);
            chart.needs_redraw = true;
            println!("Toggled Price pane");
        } else {
            println!("Cannot hide last visible pane!");
        }
    }

    // Future: Toggle indicator panes with 'I' key
    if keyboard.just_pressed(KeyCode::KeyI) {
        // Loop through indicator panes and toggle
        println!("Indicator toggle not yet implemented");
    }
}
```

#### Step 3: Layout Recalculation

**File**: `types.rs`

**Modify `calculate_pane_layouts()`:**
```rust
pub fn calculate_pane_layouts(panes: &mut [Pane], total_area: Rect, visible_candle_count: usize) {
    if panes.is_empty() {
        return;
    }

    // Calculate total height percentage from visible panes only
    let visible_panes: Vec<&Pane> = panes.iter().filter(|p| p.visible).collect();
    let total_visible_percent: f32 = visible_panes.iter()
        .map(|p| p.height_percent)
        .sum();

    if total_visible_percent <= 0.0 {
        return;
    }

    // Start from the top
    let mut current_y = total_area.max.y;
    let total_height = total_area.height();

    for pane in panes.iter_mut() {
        if !pane.visible {
            // Hidden panes get zero-size viewport
            pane.space.viewport = Rect::from_corners(Vec2::ZERO, Vec2::ZERO);
            continue;
        }

        // Normalize height percentage relative to visible panes only
        let normalized_percent = pane.height_percent / total_visible_percent;
        let pane_height = total_height * normalized_percent;
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
```

#### Step 4: Rendering Updates

**File**: `rendering.rs`

**Update all render functions to check visibility:**

**render_candlesticks():**
```rust
// Find the Price pane
let price_pane = chart.panes.iter()
    .find(|p| matches!(p.id, PaneId::Price) && p.visible);  // Check visible

if price_pane.is_none() {
    return;  // Skip if hidden
}
```

**render_volume_bars():**
```rust
// Find the Volume pane
let volume_pane = chart.panes.iter()
    .find(|p| matches!(p.id, PaneId::Volume) && p.visible);  // Check visible

if volume_pane.is_none() {
    return;  // Skip if hidden
}
```

**render_grid_and_axes():**
```rust
// Filter to visible panes only
let visible_panes: Vec<&Pane> = chart.panes.iter()
    .filter(|p| p.visible)
    .collect();

if visible_panes.is_empty() {
    return;
}

// Use visible_panes instead of chart.panes
let first_pane = visible_panes[0];
let last_pane = visible_panes[visible_panes.len() - 1];

// ... rest of rendering logic using visible_panes
```

**render_crosshair():**
```rust
// Check if mouse is within any visible pane
let mouse_in_chart = chart.panes.iter()
    .filter(|p| p.visible)
    .any(|pane| pane.space.viewport.contains(interaction.mouse_pos));

// ... use visible panes for horizontal lines
for pane in chart.panes.iter().filter(|p| p.visible) {
    // Draw crosshair line
}
```

#### Step 5: Visual Feedback

**Option A: Status Message**
When a pane is hidden, show a small message:
- Position: Bottom-right corner
- Text: "Press V to show Volume pane"
- Fade out after 2 seconds

**Option B: Collapsed Bar**
When pane is hidden, show a thin 20px bar:
- Shows pane name
- Click to restore
- Hover changes cursor

**Option C: Just Keyboard (Simplest)**
- No visual indicator
- User must remember 'V' / 'P' keys
- Print to console when toggled

#### Step 6: System Registration

**File**: `main.rs`

Add to Update systems:
```rust
.add_systems(Update, handle_keyboard_input)
```

---

## Implementation Order

### Stage 1: Visual Separation (Easiest, Immediate Improvement)
**Estimated Time**: 30 minutes

**Changes:**
- Thicker separator (6-8px) with more opaque color
- Add padding between panes (reduce viewport by 10px on each side of separator)
- Add pane title headers ("PRICE" / "VOLUME")
- Improve separator visibility

**Files Modified:**
- `rendering.rs`: Update separator rendering
- `types.rs`: Modify `calculate_pane_layouts()` to add padding
- `rendering.rs`: Add pane title rendering

**Testing:**
- Visual inspection
- Verify grids don't overlap
- Verify labels are readable

---

### Stage 2: Pane Toggle (Medium Complexity, Useful Feature)
**Estimated Time**: 45 minutes

**Changes:**
- Add `visible` field to `Pane` struct
- Add `toggle_pane()` method to `Chart`
- Create `handle_keyboard_input()` system
- Update all rendering systems to respect visibility
- Modify `calculate_pane_layouts()` to handle hidden panes

**Files Modified:**
- `types.rs`: Add visibility field and methods
- `interaction.rs`: Add keyboard handler
- `rendering.rs`: Update all render functions
- `main.rs`: Register new system

**Testing:**
- Press 'V' to toggle volume pane
- Press 'P' to toggle price pane
- Verify layout recalculates correctly
- Verify can't hide all panes

---

### Stage 3: Pane Resize (Most Complex, Polish Feature)
**Estimated Time**: 1.5 hours

**Changes:**
- Add `Separator` component
- Extend `InteractionState` with resize state
- Create `handle_pane_resize()` system
- Add hover detection and cursor change
- Implement drag logic with constraints

**Files Modified:**
- `types.rs`: Add Separator component, extend InteractionState
- `rendering.rs`: Add Separator component to separators
- `interaction.rs`: Add resize handler
- `main.rs`: Register new system

**Testing:**
- Hover over separator (cursor changes)
- Drag separator up/down
- Verify minimum height constraints
- Verify smooth dragging
- Test edge cases (dragging beyond limits)

---

## Questions / Decisions Needed

### 1. Separator Style
**Options:**
- A) Simple thick line (6-8px solid color)
- B) Textured grip (dots or lines pattern)
- C) Different color (blue/orange to match theme)
- D) Gradient separator

**Recommendation**: Start with (A), can enhance later

---

### 2. Pane Headers
**Options:**
- A) Top-left "PRICE" / "VOLUME" text
- B) Centered header bar with pane name
- C) Small icon + text
- D) No header, just rely on separator

**Recommendation**: (A) - Simple and unobtrusive

---

### 3. Toggle UI
**Options:**
- A) Just keyboard shortcuts (V/P keys)
- B) Add on-screen buttons in top-right corner
- C) Right-click context menu
- D) Settings panel

**Recommendation**: (A) for MVP, (B) as enhancement

---

### 4. Resize Constraints
**Questions:**
- Minimum pane height: 10% okay?
- Should Price pane have larger minimum (30%)?
- Should we save resize state (persist on restart)?
- Should there be preset layouts (50/50, 70/30, 80/20)?

**Recommendation**:
- Min height: 10% for all panes
- No persistence initially (can add later)
- No presets initially (can add later)

---

### 5. Visual Feedback for Hidden Panes
**Options:**
- A) Console message only
- B) On-screen toast/notification
- C) Collapsed bar (clickable to restore)
- D) Status indicator showing visible panes

**Recommendation**: (A) for MVP, (C) as nice enhancement

---

## Success Criteria

### Stage 1: Visual Separation
- [ ] Separator is clearly visible
- [ ] No grid lines touching separator
- [ ] Pane headers visible
- [ ] Y-axis labels not confusing between panes

### Stage 2: Pane Toggle
- [ ] 'V' key toggles Volume pane
- [ ] 'P' key toggles Price pane
- [ ] Layout recalculates when pane hidden
- [ ] Cannot hide all panes
- [ ] Crosshair and grid only span visible panes

### Stage 3: Pane Resize
- [ ] Cursor changes to NS-RESIZE when hovering separator
- [ ] Can drag separator smoothly
- [ ] Minimum height constraints enforced
- [ ] Layout updates in real-time during drag
- [ ] Cursor resets when releasing drag

---

## Next Steps

**Decision Required:**
Which stages to implement?
- All three stages?
- Just Stage 1 + Stage 2?
- Just Stage 1 first, then decide?

Please review and let me know which approach you prefer!
