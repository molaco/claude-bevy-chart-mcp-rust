use bevy::prelude::*;

use crate::interaction::state::{InteractionMode, InteractionState};
use crate::types::{CandleData, IndicatorState, PaneManager, ViewportState, right_spacing_candles};

// ============================================================================
// PAN HANDLER
// ============================================================================

/// Handle panning (horizontal scrolling) while in Panning mode.
/// Converts mouse drag delta to candle offset and updates viewport.
pub fn handle_pan(
    candle_data: Res<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut pane_manager: ResMut<PaneManager>,
    indicator_state: Res<IndicatorState>,
    mut interaction: ResMut<InteractionState>,
) {
    // Only process when in panning mode
    let InteractionMode::Panning { drag_start } = interaction.mode else {
        return;
    };

    if pane_manager.panes.is_empty() {
        return;
    }

    let delta_x = interaction.mouse_pos.x - drag_start.x;
    let candle_width_px = pane_manager.panes[0].space.candle_width_px;
    let candles_moved = -(delta_x / candle_width_px) as i32;

    if candles_moved != 0 {
        // Update shared X-axis state
        let new_start = (viewport_state.visible_candle_start as i32 + candles_moved).max(0) as usize;
        let spacing = right_spacing_candles(viewport_state.visible_candle_count);
        viewport_state.visible_candle_start = new_start.min(
            (candle_data.candles.len() + spacing).saturating_sub(viewport_state.visible_candle_count),
        );

        // Update Y-axis bounds for all panes
        pane_manager.update_pane_bounds(
            &candle_data.candles,
            &indicator_state.indicators,
            viewport_state.visible_candle_start,
            viewport_state.visible_candle_count,
        );

        viewport_state.needs_redraw = true;

        // Update drag start to current position for next frame
        interaction.mode = InteractionMode::Panning {
            drag_start: interaction.mouse_pos,
        };
    }
}
