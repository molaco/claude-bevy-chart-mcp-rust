use bevy::prelude::*;

use crate::config::InteractionConfig;
use crate::interaction::state::{InteractionMode, InteractionState};
use crate::types::{CandleData, IndicatorState, PaneManager, ViewportState};

// ============================================================================
// RESIZE HANDLER
// ============================================================================

/// Handle pane resizing while in ResizingPane mode.
/// Adjusts pane heights based on drag delta while respecting constraints.
pub fn handle_resize(
    candle_data: Res<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut pane_manager: ResMut<PaneManager>,
    indicator_state: Res<IndicatorState>,
    interaction: Res<InteractionState>,
    interaction_config: Res<InteractionConfig>,
) {
    // Only process when in resizing mode
    let InteractionMode::ResizingPane {
        gap_index,
        drag_start,
        ref start_heights,
    } = interaction.mode
    else {
        return;
    };

    let delta_y = interaction.mouse_pos.y - drag_start.y;

    // Calculate available height (excluding gaps)
    let num_gaps = pane_manager.panes.len() - 1;
    let available_height =
        viewport_state.total_area.height() - (num_gaps as f32 * pane_manager.separator_gap);

    // Convert pixel movement to percentage change
    let delta_percent = -delta_y / available_height; // Negative because Y is flipped

    // Get original heights
    let orig_above = start_heights[gap_index];
    let orig_below = start_heights[gap_index + 1];

    // Calculate new heights with constraints from config
    let new_above = (orig_above + delta_percent).clamp(
        interaction_config.min_pane_height,
        interaction_config.max_pane_height,
    );
    let new_below = (orig_below - delta_percent).clamp(
        interaction_config.min_pane_height,
        interaction_config.max_pane_height,
    );

    // Check if both constraints are satisfied
    let total_change = (new_above - orig_above).abs() + (new_below - orig_below).abs();
    if total_change > crate::config::RESIZE_SENSITIVITY_THRESHOLD {
        pane_manager.panes[gap_index].height_percent = new_above;
        pane_manager.panes[gap_index + 1].height_percent = new_below;

        // Recalculate layouts
        pane_manager.calculate_layouts(viewport_state.total_area, viewport_state.visible_candle_count);

        // Update Y-axis bounds
        pane_manager.update_pane_bounds(
            &candle_data.candles,
            &indicator_state.indicators,
            viewport_state.visible_candle_start,
            viewport_state.visible_candle_count,
        );

        viewport_state.needs_redraw = true;
    }
}
