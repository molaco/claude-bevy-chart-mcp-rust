use bevy::prelude::*;

use crate::config::InteractionConfig;
use crate::types::{
    CandleData, IndicatorState, Pane, PaneId, PaneManager, PaneType, ViewportState,
    VolumeToggleState,
};

// ============================================================================
// KEYBOARD HANDLERS
// ============================================================================

/// Toggle volume pane visibility with 'V' key.
pub fn toggle_volume_pane(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_state: ResMut<VolumeToggleState>,
    candle_data: Res<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut pane_manager: ResMut<PaneManager>,
    indicator_state: Res<IndicatorState>,
    interaction_config: Res<InteractionConfig>,
) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }

    toggle_state.visible = !toggle_state.visible;

    if toggle_state.visible {
        // Add volume pane back (if not already present)
        let has_volume = pane_manager
            .panes
            .iter()
            .any(|p| matches!(p.id, PaneId::Volume));

        if !has_volume {
            // Insert volume pane after price pane (using config for default height)
            let volume_pane = Pane::new(
                PaneId::Volume,
                PaneType::Volume,
                interaction_config.default_volume_pane_height,
                Rect::default(),
                viewport_state.visible_candle_count,
            );
            pane_manager.panes.push(volume_pane);

            // Adjust price pane height (using config for default height)
            if let Some(price_pane) = pane_manager.find_pane_mut(PaneId::Price) {
                price_pane.height_percent = interaction_config.default_price_pane_height;
            }
        }
    } else {
        // Remove volume pane
        pane_manager
            .panes
            .retain(|p| !matches!(p.id, PaneId::Volume));

        // Give price pane 100% height
        if let Some(price_pane) = pane_manager.find_pane_mut(PaneId::Price) {
            price_pane.height_percent = crate::config::FULL_PANE_HEIGHT;
        }
    }

    // Recalculate pane layouts
    pane_manager.calculate_layouts(viewport_state.total_area, viewport_state.visible_candle_count);

    // Update Y-axis bounds for all panes
    pane_manager.update_pane_bounds(
        &candle_data.candles,
        &indicator_state.indicators,
        viewport_state.visible_candle_start,
        viewport_state.visible_candle_count,
    );

    // Trigger redraw
    viewport_state.needs_redraw = true;
}

/// Toggle SMA indicator visibility with number keys.
/// Keys: 1 = SMA-20, 2 = SMA-50, 3 = SMA-200, S = Toggle all SMAs
pub fn toggle_sma_indicators(
    keys: Res<ButtonInput<KeyCode>>,
    candle_data: Res<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut pane_manager: ResMut<PaneManager>,
    mut indicator_state: ResMut<IndicatorState>,
) {
    let mut toggled = false;

    // Toggle individual SMAs with number keys
    if keys.just_pressed(KeyCode::Digit1) {
        if let Some(sma) = indicator_state.indicators.get_mut(0) {
            sma.visible = !sma.visible;
            toggled = true;
        }
    }

    if keys.just_pressed(KeyCode::Digit2) {
        if let Some(sma) = indicator_state.indicators.get_mut(1) {
            sma.visible = !sma.visible;
            toggled = true;
        }
    }

    if keys.just_pressed(KeyCode::Digit3) {
        if let Some(sma) = indicator_state.indicators.get_mut(2) {
            sma.visible = !sma.visible;
            toggled = true;
        }
    }

    // Toggle all SMAs with 'S' key
    if keys.just_pressed(KeyCode::KeyS) {
        indicator_state.toggle_all();
        toggled = true;
    }

    // If any toggle occurred, update bounds and trigger redraw
    if toggled {
        // Update Y-axis bounds (will respect new visibility state)
        pane_manager.update_pane_bounds(
            &candle_data.candles,
            &indicator_state.indicators,
            viewport_state.visible_candle_start,
            viewport_state.visible_candle_count,
        );

        viewport_state.needs_redraw = true;
    }
}
