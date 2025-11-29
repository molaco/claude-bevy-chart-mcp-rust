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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // InteractionMode TESTS
    // ========================================================================

    #[test]
    fn test_interaction_mode_idle_is_default() {
        let mode = InteractionMode::default();
        assert!(matches!(mode, InteractionMode::Idle));
    }

    #[test]
    fn test_interaction_mode_idle_not_active() {
        let mode = InteractionMode::Idle;
        assert!(!mode.is_active(), "Idle should not be active");
        assert!(!mode.is_panning(), "Idle should not be panning");
        assert!(!mode.is_resizing(), "Idle should not be resizing");
        assert!(!mode.is_hovering_gap(), "Idle should not be hovering");
        assert_eq!(mode.gap_index(), None, "Idle should have no gap index");
    }

    #[test]
    fn test_interaction_mode_hovering_gap() {
        let mode = InteractionMode::HoveringGap { gap_index: 0 };
        assert!(!mode.is_active(), "HoveringGap should not be active");
        assert!(!mode.is_panning(), "HoveringGap should not be panning");
        assert!(!mode.is_resizing(), "HoveringGap should not be resizing");
        assert!(mode.is_hovering_gap(), "HoveringGap should report hovering");
        assert_eq!(mode.gap_index(), Some(0), "HoveringGap should report gap index");
    }

    #[test]
    fn test_interaction_mode_panning_is_active() {
        let mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 200.0),
        };
        assert!(mode.is_active(), "Panning should be active");
        assert!(mode.is_panning(), "Panning should report panning");
        assert!(!mode.is_resizing(), "Panning should not be resizing");
        assert!(!mode.is_hovering_gap(), "Panning should not be hovering");
        assert_eq!(mode.gap_index(), None, "Panning should have no gap index");
    }

    #[test]
    fn test_interaction_mode_resizing_is_active() {
        let mode = InteractionMode::ResizingPane {
            gap_index: 1,
            drag_start: Vec2::new(100.0, 200.0),
            start_heights: vec![0.7, 0.3],
        };
        assert!(mode.is_active(), "ResizingPane should be active");
        assert!(!mode.is_panning(), "ResizingPane should not be panning");
        assert!(mode.is_resizing(), "ResizingPane should report resizing");
        assert!(!mode.is_hovering_gap(), "ResizingPane should not be hovering gap");
        assert_eq!(mode.gap_index(), Some(1), "ResizingPane should report gap index");
    }

    #[test]
    fn test_interaction_mode_different_gap_indices() {
        for gap_idx in 0..5 {
            let hovering = InteractionMode::HoveringGap { gap_index: gap_idx };
            assert_eq!(hovering.gap_index(), Some(gap_idx));

            let resizing = InteractionMode::ResizingPane {
                gap_index: gap_idx,
                drag_start: Vec2::ZERO,
                start_heights: vec![],
            };
            assert_eq!(resizing.gap_index(), Some(gap_idx));
        }
    }

    // ========================================================================
    // InteractionState TESTS
    // ========================================================================

    #[test]
    fn test_interaction_state_default() {
        let state = InteractionState::default();
        assert!(matches!(state.mode, InteractionMode::Idle));
        assert_eq!(state.mouse_pos, Vec2::ZERO);
    }

    #[test]
    fn test_interaction_state_to_idle() {
        let mut state = InteractionState {
            mode: InteractionMode::Panning {
                drag_start: Vec2::new(100.0, 100.0),
            },
            mouse_pos: Vec2::new(200.0, 200.0),
        };

        state.to_idle();

        assert!(matches!(state.mode, InteractionMode::Idle));
        // Mouse position should be preserved
        assert_eq!(state.mouse_pos, Vec2::new(200.0, 200.0));
    }

    #[test]
    fn test_interaction_state_to_hovering_gap() {
        let mut state = InteractionState::default();

        state.to_hovering_gap(2);

        assert!(matches!(state.mode, InteractionMode::HoveringGap { gap_index: 2 }));
    }

    #[test]
    fn test_interaction_state_to_panning() {
        let mut state = InteractionState {
            mode: InteractionMode::Idle,
            mouse_pos: Vec2::new(150.0, 250.0),
        };

        state.to_panning();

        match state.mode {
            InteractionMode::Panning { drag_start } => {
                assert_eq!(drag_start, Vec2::new(150.0, 250.0), "Drag start should use current mouse_pos");
            }
            _ => panic!("Should be in Panning mode"),
        }
    }

    #[test]
    fn test_interaction_state_to_resizing_pane() {
        let mut state = InteractionState {
            mode: InteractionMode::HoveringGap { gap_index: 0 },
            mouse_pos: Vec2::new(400.0, 300.0),
        };

        let start_heights = vec![0.7, 0.3];
        state.to_resizing_pane(0, start_heights.clone());

        match &state.mode {
            InteractionMode::ResizingPane {
                gap_index,
                drag_start,
                start_heights: heights,
            } => {
                assert_eq!(*gap_index, 0);
                assert_eq!(*drag_start, Vec2::new(400.0, 300.0));
                assert_eq!(*heights, start_heights);
            }
            _ => panic!("Should be in ResizingPane mode"),
        }
    }

    #[test]
    fn test_interaction_state_should_show_resize_cursor() {
        let mut state = InteractionState::default();

        // Idle - no resize cursor
        state.mode = InteractionMode::Idle;
        assert!(!state.should_show_resize_cursor());

        // HoveringGap - show resize cursor
        state.mode = InteractionMode::HoveringGap { gap_index: 0 };
        assert!(state.should_show_resize_cursor());

        // Panning - no resize cursor
        state.mode = InteractionMode::Panning {
            drag_start: Vec2::ZERO,
        };
        assert!(!state.should_show_resize_cursor());

        // ResizingPane - show resize cursor
        state.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::ZERO,
            start_heights: vec![],
        };
        assert!(state.should_show_resize_cursor());
    }

    // ========================================================================
    // STATE TRANSITION SEQUENCE TESTS
    // ========================================================================

    #[test]
    fn test_state_transition_idle_to_panning_to_idle() {
        let mut state = InteractionState {
            mode: InteractionMode::Idle,
            mouse_pos: Vec2::new(100.0, 100.0),
        };

        // Start panning
        state.to_panning();
        assert!(state.mode.is_panning());

        // Update mouse position (simulating drag)
        state.mouse_pos = Vec2::new(200.0, 100.0);

        // Release - back to idle
        state.to_idle();
        assert!(matches!(state.mode, InteractionMode::Idle));
    }

    #[test]
    fn test_state_transition_idle_to_hovering_to_resizing_to_idle() {
        let mut state = InteractionState::default();

        // Mouse enters gap
        state.to_hovering_gap(0);
        assert!(state.mode.is_hovering_gap());

        // Click to start resizing
        state.mouse_pos = Vec2::new(400.0, 300.0);
        state.to_resizing_pane(0, vec![0.7, 0.3]);
        assert!(state.mode.is_resizing());

        // Release - back to idle
        state.to_idle();
        assert!(matches!(state.mode, InteractionMode::Idle));
    }

    #[test]
    fn test_state_transition_hovering_gap_switch() {
        let mut state = InteractionState::default();

        // Hover over gap 0
        state.to_hovering_gap(0);
        assert_eq!(state.mode.gap_index(), Some(0));

        // Mouse moves to gap 1
        state.to_hovering_gap(1);
        assert_eq!(state.mode.gap_index(), Some(1));

        // Mouse leaves all gaps
        state.to_idle();
        assert_eq!(state.mode.gap_index(), None);
    }
}
