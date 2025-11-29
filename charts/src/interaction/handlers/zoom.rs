use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;

use crate::config::InteractionConfig;
use crate::interaction::state::InteractionState;
use crate::types::{CandleData, IndicatorState, PaneManager, ViewportState, right_spacing_candles};

// ============================================================================
// ZOOM HANDLER
// ============================================================================

/// Handle mouse wheel zoom events.
/// Zooms the viewport while maintaining focus on the mouse position.
pub fn handle_zoom(
    candle_data: Res<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut pane_manager: ResMut<PaneManager>,
    indicator_state: Res<IndicatorState>,
    interaction: Res<InteractionState>,
    interaction_config: Res<InteractionConfig>,
    #[allow(deprecated)] mut mouse_wheel: EventReader<MouseWheel>,
) {
    // Don't process zoom while resizing
    if interaction.mode.is_resizing() {
        return;
    }

    // Check if mouse is in any pane
    let mouse_in_pane = pane_manager
        .panes
        .iter()
        .any(|pane| pane.space.viewport.contains(interaction.mouse_pos));

    if !mouse_in_pane || pane_manager.panes.is_empty() {
        // Consume events even if not in pane to prevent accumulation
        mouse_wheel.clear();
        return;
    }

    for event in mouse_wheel.read() {
        let zoom_factor = if event.y > 0.0 {
            interaction_config.zoom_in_factor
        } else {
            interaction_config.zoom_out_factor
        };

        // Calculate focus candle using first pane
        let (focus_candle, _) = pane_manager.panes[0].space.from_world(
            interaction.mouse_pos,
            viewport_state.visible_candle_start,
            viewport_state.visible_candle_count,
        );

        // Update shared X-axis state (zoom)
        let old_count = viewport_state.visible_candle_count;
        let new_count = ((old_count as f32 * zoom_factor).clamp(
            interaction_config.min_visible_candles,
            interaction_config.max_visible_candles,
        ) as usize)
            .min(candle_data.candles.len());

        let focus_offset = focus_candle.saturating_sub(viewport_state.visible_candle_start);
        let focus_percent = focus_offset as f32 / old_count as f32;

        let spacing = right_spacing_candles(new_count);
        viewport_state.visible_candle_start = focus_candle
            .saturating_sub((new_count as f32 * focus_percent) as usize)
            .min((candle_data.candles.len() + spacing).saturating_sub(new_count));
        viewport_state.visible_candle_count = new_count;

        // Update Y-axis bounds for all panes
        pane_manager.update_pane_bounds(
            &candle_data.candles,
            &indicator_state.indicators,
            viewport_state.visible_candle_start,
            viewport_state.visible_candle_count,
        );

        viewport_state.needs_redraw = true;
    }
}
