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
fn detect_hovered_gap(mouse_y: f32, pane_manager: &PaneManager) -> Option<usize> {
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
