use bevy::prelude::*;

use crate::interaction::state::{InteractionMode, InteractionState};
use crate::types::PaneManager;

// ============================================================================
// STATE TRANSITIONS
// ============================================================================

/// Update interaction mode based on input events.
/// This is the central state machine transition logic.
///
/// Transition table:
/// | From         | Event              | To             | Guard Condition          |
/// |--------------|--------------------|----------------|--------------------------|
/// | Idle         | Mouse enters gap   | HoveringGap    | mouse_y in gap bounds    |
/// | Idle         | Left click in pane | Panning        | mouse_in_pane && !in_gap |
/// | HoveringGap  | Mouse leaves gap   | Idle           | mouse_y not in gap       |
/// | HoveringGap  | Left click         | ResizingPane   | just_pressed(Left)       |
/// | Panning      | Left release       | Idle           | just_released(Left)      |
/// | ResizingPane | Left release       | Idle           | just_released(Left)      |
pub fn update_interaction_mode(
    mut interaction: ResMut<InteractionState>,
    pane_manager: Res<PaneManager>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    let mouse_y = interaction.mouse_pos.y;

    // Detect which gap (if any) the mouse is hovering over
    let hovered_gap = detect_hovered_gap(mouse_y, &pane_manager);

    // Check if mouse is in any pane
    let mouse_in_pane = pane_manager
        .panes
        .iter()
        .any(|pane| pane.space.viewport.contains(interaction.mouse_pos));

    // State machine transitions
    match &interaction.mode {
        InteractionMode::Idle => {
            // Idle → HoveringGap: Mouse enters a gap
            if let Some(gap_index) = hovered_gap {
                interaction.to_hovering_gap(gap_index);
            }
            // Idle → Panning: Left click in pane (not in gap)
            else if mouse_button.just_pressed(MouseButton::Left) && mouse_in_pane {
                interaction.to_panning();
            }
        }

        InteractionMode::HoveringGap { gap_index } => {
            // Copy gap_index to avoid borrow issues
            let current_gap = *gap_index;

            // HoveringGap → ResizingPane: Left click while hovering
            if mouse_button.just_pressed(MouseButton::Left) {
                let start_heights: Vec<f32> = pane_manager
                    .panes
                    .iter()
                    .map(|p| p.height_percent)
                    .collect();
                interaction.to_resizing_pane(current_gap, start_heights);
            }
            // HoveringGap → Idle: Mouse leaves gap
            else if hovered_gap != Some(current_gap) {
                // Check if moved to a different gap
                if let Some(new_gap) = hovered_gap {
                    interaction.to_hovering_gap(new_gap);
                } else {
                    interaction.to_idle();
                }
            }
        }

        InteractionMode::Panning { .. } => {
            // Panning → Idle: Left button released
            if mouse_button.just_released(MouseButton::Left) {
                interaction.to_idle();
            }
        }

        InteractionMode::ResizingPane { .. } => {
            // ResizingPane → Idle: Left button released
            if mouse_button.just_released(MouseButton::Left) {
                interaction.to_idle();
            }
        }
    }
}

/// Detect which gap (if any) the mouse Y position is hovering over.
///
/// Returns the index of the gap (between panes[i] and panes[i+1]) if the
/// mouse Y coordinate is within that gap region.
pub(crate) fn detect_hovered_gap(mouse_y: f32, pane_manager: &PaneManager) -> Option<usize> {
    for i in 0..pane_manager.panes.len().saturating_sub(1) {
        let pane_bottom = pane_manager.panes[i].space.viewport.min.y;
        let next_pane_top = pane_manager.panes[i + 1].space.viewport.max.y;

        // Check if mouse is in the gap between panes
        if mouse_y <= pane_bottom && mouse_y >= next_pane_top {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Rect, Vec2};
    use crate::panes::{Pane, PaneId, PaneType};

    /// Create a test pane with specific viewport bounds
    fn create_test_pane_with_viewport(id: PaneId, pane_type: PaneType, height: f32, viewport: Rect) -> Pane {
        let mut pane = Pane::new(id, pane_type, height, viewport, 50);
        pane.space.viewport = viewport;
        pane
    }

    /// Create a standard two-pane manager with known viewport positions
    fn create_two_pane_manager_with_gap() -> PaneManager {
        // Price pane: top of screen (higher Y)
        // viewport from y=310 to y=600 (top pane)
        let price_pane = create_test_pane_with_viewport(
            PaneId::Price,
            PaneType::Price,
            0.7,
            Rect::from_corners(Vec2::new(0.0, 310.0), Vec2::new(800.0, 600.0)),
        );

        // Volume pane: bottom of screen (lower Y)
        // viewport from y=0 to y=290 (bottom pane)
        let volume_pane = create_test_pane_with_viewport(
            PaneId::Volume,
            PaneType::Volume,
            0.3,
            Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 290.0)),
        );

        // Gap is from y=290 to y=310 (20px gap)
        PaneManager::new_with_gap(vec![price_pane, volume_pane], 20.0)
    }

    // ========================================================================
    // detect_hovered_gap() TESTS
    // ========================================================================

    #[test]
    fn test_detect_hovered_gap_in_gap() {
        let manager = create_two_pane_manager_with_gap();

        // Mouse Y at 300 is in the gap (between 290 and 310)
        let result = detect_hovered_gap(300.0, &manager);
        assert_eq!(result, Some(0), "Should detect gap 0 when mouse is in the gap");
    }

    #[test]
    fn test_detect_hovered_gap_at_gap_boundaries() {
        let manager = create_two_pane_manager_with_gap();

        // At lower boundary of gap (top of bottom pane)
        let result = detect_hovered_gap(290.0, &manager);
        assert_eq!(result, Some(0), "Should detect gap at lower boundary");

        // At upper boundary of gap (bottom of top pane)
        let result = detect_hovered_gap(310.0, &manager);
        assert_eq!(result, Some(0), "Should detect gap at upper boundary");
    }

    #[test]
    fn test_detect_hovered_gap_in_pane() {
        let manager = create_two_pane_manager_with_gap();

        // Mouse in price pane (y > 310)
        let result = detect_hovered_gap(450.0, &manager);
        assert_eq!(result, None, "Should not detect gap when in price pane");

        // Mouse in volume pane (y < 290)
        let result = detect_hovered_gap(150.0, &manager);
        assert_eq!(result, None, "Should not detect gap when in volume pane");
    }

    #[test]
    fn test_detect_hovered_gap_single_pane() {
        // Single pane - no gaps
        let pane = create_test_pane_with_viewport(
            PaneId::Price,
            PaneType::Price,
            1.0,
            Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0)),
        );
        let manager = PaneManager::new(vec![pane]);

        let result = detect_hovered_gap(300.0, &manager);
        assert_eq!(result, None, "Should not detect gap with single pane");
    }

    #[test]
    fn test_detect_hovered_gap_empty_panes() {
        let manager = PaneManager::default();

        let result = detect_hovered_gap(300.0, &manager);
        assert_eq!(result, None, "Should not detect gap with no panes");
    }

    #[test]
    fn test_detect_hovered_gap_three_panes() {
        // Three panes with two gaps
        let pane1 = create_test_pane_with_viewport(
            PaneId::Price,
            PaneType::Price,
            0.5,
            Rect::from_corners(Vec2::new(0.0, 410.0), Vec2::new(800.0, 600.0)), // top
        );
        let pane2 = create_test_pane_with_viewport(
            PaneId::Indicator(0),
            PaneType::Indicator { name: "RSI".to_string() },
            0.25,
            Rect::from_corners(Vec2::new(0.0, 210.0), Vec2::new(800.0, 390.0)), // middle
        );
        let pane3 = create_test_pane_with_viewport(
            PaneId::Volume,
            PaneType::Volume,
            0.25,
            Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 190.0)), // bottom
        );

        let manager = PaneManager::new_with_gap(vec![pane1, pane2, pane3], 20.0);

        // Gap 0 is between pane1 and pane2 (y=390 to y=410)
        let result = detect_hovered_gap(400.0, &manager);
        assert_eq!(result, Some(0), "Should detect gap 0");

        // Gap 1 is between pane2 and pane3 (y=190 to y=210)
        let result = detect_hovered_gap(200.0, &manager);
        assert_eq!(result, Some(1), "Should detect gap 1");

        // In middle pane
        let result = detect_hovered_gap(300.0, &manager);
        assert_eq!(result, None, "Should not detect gap in middle pane");
    }

    #[test]
    fn test_detect_hovered_gap_extreme_y_values() {
        let manager = create_two_pane_manager_with_gap();

        // Very high Y (above all panes)
        let result = detect_hovered_gap(1000.0, &manager);
        assert_eq!(result, None, "Should not detect gap above all panes");

        // Very low Y (below all panes)
        let result = detect_hovered_gap(-100.0, &manager);
        assert_eq!(result, None, "Should not detect gap below all panes");

        // Zero
        let result = detect_hovered_gap(0.0, &manager);
        assert_eq!(result, None, "Should not detect gap at y=0");
    }

    // ========================================================================
    // STATE TRANSITION LOGIC TESTS
    // ========================================================================
    // Note: update_interaction_mode is a Bevy system, so we test the
    // transition logic through the InteractionState methods and verify
    // the detect_hovered_gap helper function works correctly.

    #[test]
    fn test_transition_logic_idle_to_hovering_gap() {
        // When detect_hovered_gap returns Some, state should transition to HoveringGap
        let manager = create_two_pane_manager_with_gap();
        let mut state = InteractionState::default();
        state.mouse_pos = Vec2::new(400.0, 300.0); // In the gap

        // Simulate the transition logic
        if let Some(gap_index) = detect_hovered_gap(state.mouse_pos.y, &manager) {
            state.to_hovering_gap(gap_index);
        }

        assert!(state.mode.is_hovering_gap());
        assert_eq!(state.mode.gap_index(), Some(0));
    }

    #[test]
    fn test_transition_logic_hovering_to_different_gap() {
        let manager = create_two_pane_manager_with_gap();
        let mut state = InteractionState::default();
        state.to_hovering_gap(0);

        // Move mouse to a position not in a gap
        state.mouse_pos = Vec2::new(400.0, 450.0);

        // Simulate the transition logic
        let current_gap = state.mode.gap_index();
        let new_gap = detect_hovered_gap(state.mouse_pos.y, &manager);

        if new_gap != current_gap {
            if let Some(gap_idx) = new_gap {
                state.to_hovering_gap(gap_idx);
            } else {
                state.to_idle();
            }
        }

        // Should transition to idle since mouse left the gap
        assert!(matches!(state.mode, InteractionMode::Idle));
    }

    #[test]
    fn test_transition_logic_hovering_to_resizing() {
        let manager = create_two_pane_manager_with_gap();
        let mut state = InteractionState::default();
        state.mouse_pos = Vec2::new(400.0, 300.0); // In the gap
        state.to_hovering_gap(0);

        // Simulate click to start resizing
        let start_heights: Vec<f32> = manager.panes.iter().map(|p| p.height_percent).collect();
        state.to_resizing_pane(0, start_heights.clone());

        assert!(state.mode.is_resizing());
        assert_eq!(state.mode.gap_index(), Some(0));
    }

    #[test]
    fn test_transition_logic_panning_requires_not_in_gap() {
        let manager = create_two_pane_manager_with_gap();
        let mut state = InteractionState::default();

        // Mouse in pane (not in gap)
        state.mouse_pos = Vec2::new(400.0, 450.0);

        let hovered_gap = detect_hovered_gap(state.mouse_pos.y, &manager);
        let mouse_in_pane = manager
            .panes
            .iter()
            .any(|pane| pane.space.viewport.contains(state.mouse_pos));

        // Should allow panning when in pane and not in gap
        if hovered_gap.is_none() && mouse_in_pane {
            state.to_panning();
        }

        assert!(state.mode.is_panning());
    }

    #[test]
    fn test_transition_logic_no_panning_when_in_gap() {
        let manager = create_two_pane_manager_with_gap();
        let mut state = InteractionState::default();

        // Mouse in gap
        state.mouse_pos = Vec2::new(400.0, 300.0);

        let hovered_gap = detect_hovered_gap(state.mouse_pos.y, &manager);

        // Should not start panning when in gap (should start hovering instead)
        if let Some(gap_index) = hovered_gap {
            state.to_hovering_gap(gap_index);
        } else {
            state.to_panning();
        }

        // Should be hovering, not panning
        assert!(state.mode.is_hovering_gap());
        assert!(!state.mode.is_panning());
    }
}
