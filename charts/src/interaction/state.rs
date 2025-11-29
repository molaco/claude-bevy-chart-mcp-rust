use bevy::prelude::*;

// ============================================================================
// INTERACTION STATE MACHINE
// ============================================================================

/// Explicit state machine for interaction modes.
/// Replaces implicit boolean flags with clear, type-safe states.
///
/// State transition diagram:
/// ```text
///                    ┌─────────────────────────────────────────┐
///                    │                                         │
///                    ▼                                         │
///               ┌────────┐  mouse enters gap  ┌─────────────┐  │
///               │  Idle  │ ─────────────────► │ HoveringGap │ ─┤ mouse leaves gap
///               └────────┘                    └─────────────┘  │
///                    │                              │          │
///     left click in  │                              │ left click
///     pane (not gap) │                              │
///                    ▼                              ▼
///               ┌─────────┐                  ┌──────────────┐
///               │ Panning │                  │ ResizingPane │
///               └─────────┘                  └──────────────┘
///                    │                              │
///          left      │                              │ left
///          release   │                              │ release
///                    │                              │
///                    └──────────────►◄──────────────┘
///                              │
///                              ▼
///                          (back to Idle)
/// ```
#[derive(Debug, Clone, Default)]
pub enum InteractionMode {
    /// No active interaction - default state
    #[default]
    Idle,

    /// Mouse is hovering over a resize gap between panes
    HoveringGap {
        /// Index of the gap (between pane[i] and pane[i+1])
        gap_index: usize,
    },

    /// User is panning (dragging) the chart horizontally
    Panning {
        /// World position where drag started
        drag_start: Vec2,
    },

    /// User is resizing panes by dragging a gap
    ResizingPane {
        /// Index of the gap being dragged
        gap_index: usize,
        /// World position where drag started
        drag_start: Vec2,
        /// Snapshot of all pane heights at drag start (for relative adjustment)
        start_heights: Vec<f32>,
    },
}

impl InteractionMode {
    /// Check if currently in any active interaction (not idle/hovering)
    pub fn is_active(&self) -> bool {
        matches!(self, InteractionMode::Panning { .. } | InteractionMode::ResizingPane { .. })
    }

    /// Check if in panning mode
    pub fn is_panning(&self) -> bool {
        matches!(self, InteractionMode::Panning { .. })
    }

    /// Check if resizing a pane
    pub fn is_resizing(&self) -> bool {
        matches!(self, InteractionMode::ResizingPane { .. })
    }

    /// Check if hovering over a gap
    pub fn is_hovering_gap(&self) -> bool {
        matches!(self, InteractionMode::HoveringGap { .. })
    }

    /// Get the gap index if hovering or resizing
    pub fn gap_index(&self) -> Option<usize> {
        match self {
            InteractionMode::HoveringGap { gap_index } => Some(*gap_index),
            InteractionMode::ResizingPane { gap_index, .. } => Some(*gap_index),
            _ => None,
        }
    }
}

// ============================================================================
// INTERACTION STATE RESOURCE
// ============================================================================

/// Interaction state resource - simplified with explicit mode enum.
///
/// Before refactoring (7 fields):
/// ```ignore
/// pub struct InteractionState {
///     pub mouse_pos: Vec2,
///     pub dragging: bool,
///     pub drag_start_pos: Vec2,
///     pub hover_resize_gap: Option<usize>,
///     pub resizing_gap: Option<usize>,
///     pub resize_start_heights: Vec<f32>,
///     pub last_crosshair_candle_index: Option<usize>,  // orphaned
/// }
/// ```
///
/// After refactoring (2 fields):
/// - All state is captured in the `mode` enum variants
/// - Impossible states are now unrepresentable
#[derive(Resource, Default)]
pub struct InteractionState {
    /// Current interaction mode (state machine state)
    pub mode: InteractionMode,
    /// Current mouse position in world coordinates
    pub mouse_pos: Vec2,
}

impl InteractionState {
    /// Transition to idle state
    pub fn to_idle(&mut self) {
        self.mode = InteractionMode::Idle;
    }

    /// Transition to hovering gap state
    pub fn to_hovering_gap(&mut self, gap_index: usize) {
        self.mode = InteractionMode::HoveringGap { gap_index };
    }

    /// Transition to panning state
    pub fn to_panning(&mut self) {
        self.mode = InteractionMode::Panning {
            drag_start: self.mouse_pos,
        };
    }

    /// Transition to resizing pane state
    pub fn to_resizing_pane(&mut self, gap_index: usize, start_heights: Vec<f32>) {
        self.mode = InteractionMode::ResizingPane {
            gap_index,
            drag_start: self.mouse_pos,
            start_heights,
        };
    }

    /// Check if should show resize cursor
    pub fn should_show_resize_cursor(&self) -> bool {
        matches!(
            self.mode,
            InteractionMode::HoveringGap { .. } | InteractionMode::ResizingPane { .. }
        )
    }
}
