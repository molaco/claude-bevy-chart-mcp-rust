use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::window::{CursorIcon, SystemCursorIcon};
use crate::config::InteractionConfig;
use crate::types::{
    CandleData, ChartDatabase, ChartMetadata, IndicatorState, InteractionState,
    MovingAverage, Pane, PaneId, PaneManager, PaneType, ViewportState, VolumeToggleState,
    right_spacing_candles,
};

// ============================================================================
// INTERACTION SYSTEMS
// ============================================================================

pub fn handle_mouse_input(
    mut commands: Commands,
    candle_data: Res<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut pane_manager: ResMut<PaneManager>,
    indicator_state: Res<IndicatorState>,
    mut interaction: ResMut<InteractionState>,
    interaction_config: Res<InteractionConfig>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    #[allow(deprecated)] mut mouse_wheel: EventReader<MouseWheel>,
    window_query: Query<(Entity, &Window)>,
) {
    let Ok((window_entity, window)) = window_query.single() else {
        return;
    };

    // Update mouse position (convert to world space)
    // Bevy's cursor_position() has (0,0) at top-left with Y down
    // World space has (0,0) at center with Y up - need to flip Y
    if let Some(cursor_pos) = window.cursor_position() {
        let window_size = Vec2::new(window.width(), window.height());
        interaction.mouse_pos = Vec2::new(
            cursor_pos.x - window_size.x / 2.0,
            window_size.y / 2.0 - cursor_pos.y,  // Flip Y axis
        );
    }

    let mouse_y = interaction.mouse_pos.y;

    // ========== PANE RESIZE: Gap Detection ==========
    interaction.hover_resize_gap = None;
    for i in 0..pane_manager.panes.len().saturating_sub(1) {
        let pane_bottom = pane_manager.panes[i].space.viewport.min.y;
        let next_pane_top = pane_manager.panes[i + 1].space.viewport.max.y;

        // Check if mouse is in the gap between panes
        if mouse_y <= pane_bottom && mouse_y >= next_pane_top {
            interaction.hover_resize_gap = Some(i);
            break;
        }
    }

    // Change cursor when hovering gap
    if interaction.hover_resize_gap.is_some() || interaction.resizing_gap.is_some() {
        commands.entity(window_entity).insert(CursorIcon::from(SystemCursorIcon::NsResize));
    } else {
        commands.entity(window_entity).insert(CursorIcon::from(SystemCursorIcon::Default));
    }

    // ========== PANE RESIZE: Start/Stop ==========
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some(gap_idx) = interaction.hover_resize_gap {
            // Start resize
            interaction.resizing_gap = Some(gap_idx);
            interaction.drag_start_pos = interaction.mouse_pos;
            interaction.resize_start_heights = pane_manager.panes.iter()
                .map(|p| p.height_percent)
                .collect();
        }
    }

    if mouse_button.just_released(MouseButton::Left) {
        if interaction.resizing_gap.is_some() {
            interaction.resizing_gap = None;
            commands.entity(window_entity).insert(CursorIcon::from(SystemCursorIcon::Default));
        }
    }

    // ========== PANE RESIZE: Drag Logic ==========
    if let Some(gap_idx) = interaction.resizing_gap {
        let delta_y = interaction.mouse_pos.y - interaction.drag_start_pos.y;

        // Calculate available height (excluding gaps)
        let num_gaps = pane_manager.panes.len() - 1;
        let available_height = viewport_state.total_area.height() - (num_gaps as f32 * pane_manager.separator_gap);

        // Convert pixel movement to percentage change
        let delta_percent = -delta_y / available_height; // Negative because Y is flipped

        // Get original heights
        let orig_above = interaction.resize_start_heights[gap_idx];
        let orig_below = interaction.resize_start_heights[gap_idx + 1];

        // Calculate new heights with constraints from config
        let new_above = (orig_above + delta_percent).clamp(interaction_config.min_pane_height, interaction_config.max_pane_height);
        let new_below = (orig_below - delta_percent).clamp(interaction_config.min_pane_height, interaction_config.max_pane_height);

        // Check if both constraints are satisfied
        let total_change = (new_above - orig_above).abs() + (new_below - orig_below).abs();
        if total_change > crate::config::RESIZE_SENSITIVITY_THRESHOLD {
            pane_manager.panes[gap_idx].height_percent = new_above;
            pane_manager.panes[gap_idx + 1].height_percent = new_below;

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

        return; // Skip pan/zoom while resizing
    }

    // Check if mouse is in any pane
    let mouse_in_pane = pane_manager.panes.iter()
        .any(|pane| pane.space.viewport.contains(interaction.mouse_pos));

    // Pan: Left mouse button drag
    if mouse_button.just_pressed(MouseButton::Left) {
        if mouse_in_pane {
            interaction.dragging = true;
            interaction.drag_start_pos = interaction.mouse_pos;
        }
    }

    if mouse_button.just_released(MouseButton::Left) {
        interaction.dragging = false;
    }

    if interaction.dragging && !pane_manager.panes.is_empty() {
        let delta_x = interaction.mouse_pos.x - interaction.drag_start_pos.x;
        let candle_width_px = pane_manager.panes[0].space.candle_width_px;
        let candles_moved = -(delta_x / candle_width_px) as i32;

        if candles_moved != 0 {
            // Update shared X-axis state
            let new_start = (viewport_state.visible_candle_start as i32 + candles_moved).max(0) as usize;
            let spacing = right_spacing_candles(viewport_state.visible_candle_count);
            viewport_state.visible_candle_start = new_start.min(
                (candle_data.candles.len() + spacing).saturating_sub(viewport_state.visible_candle_count)
            );

            // Update Y-axis bounds for all panes
            pane_manager.update_pane_bounds(
                &candle_data.candles,
                &indicator_state.indicators,
                viewport_state.visible_candle_start,
                viewport_state.visible_candle_count,
            );

            viewport_state.needs_redraw = true;
            interaction.drag_start_pos = interaction.mouse_pos;
        }
    }

    // Zoom: Mouse wheel
    for event in mouse_wheel.read() {
        if mouse_in_pane && !pane_manager.panes.is_empty() {
            let zoom_factor = if event.y > 0.0 {
                interaction_config.zoom_in_factor
            } else {
                interaction_config.zoom_out_factor
            };

            // Calculate focus candle using first pane
            let (focus_candle, _) = pane_manager.panes[0].space.from_world(
                interaction.mouse_pos,
                viewport_state.visible_candle_start,
                viewport_state.visible_candle_count
            );

            // Update shared X-axis state (zoom)
            let old_count = viewport_state.visible_candle_count;
            let new_count = ((old_count as f32 * zoom_factor).clamp(
                interaction_config.min_visible_candles,
                interaction_config.max_visible_candles
            ) as usize).min(candle_data.candles.len());

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

            println!(
                "Zoomed: showing {} candles starting from {}",
                viewport_state.visible_candle_count,
                viewport_state.visible_candle_start
            );
        }
    }
}

pub fn check_lazy_load(
    mut candle_data: ResMut<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut indicator_state: ResMut<IndicatorState>,
    chart_metadata: Res<ChartMetadata>,
    interaction_config: Res<InteractionConfig>,
    db: Res<ChartDatabase>,
) {
    if viewport_state.loading {
        return;  // Already loading
    }

    let start_idx = viewport_state.visible_candle_start;
    let end_idx = start_idx + viewport_state.visible_candle_count;

    // Load more historical data when scrolling left
    if start_idx < interaction_config.lazy_load_threshold && candle_data.candles.first().is_some() {
        viewport_state.loading = true;

        let load_count = interaction_config.lazy_load_batch_size;
        let load_end_time = candle_data.candles.first().unwrap().time;

        // Calculate start time based on timeframe
        let interval_ms = chart_metadata.interval_ms();
        let load_start_time = load_end_time - (load_count as i64 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart_metadata.ticker_id,
            &chart_metadata.timeframe,
            load_start_time,
            load_end_time - 1, // Exclude the first candle we already have
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} historical candles", new_candles.len());

                // Prepend new candles
                let new_len = new_candles.len();
                let mut combined = new_candles;
                combined.append(&mut candle_data.candles);
                candle_data.candles = combined;

                // Adjust visible_start to maintain view
                viewport_state.visible_candle_start += new_len;

                // INCREMENTAL: Only calculate MA for NEW candles
                println!("Recalculating indicators for {} new candles (prepend)", new_len);

                // Update indicators
                for indicator in indicator_state.indicators.iter_mut() {
                    if indicator.name.starts_with("SMA") {
                        indicator.calculate_sma_prepend(&candle_data.candles, new_len);
                    } else if indicator.name.starts_with("EMA") {
                        // EMA requires recursive calculation, must recalculate all
                        println!("Warning: EMA requires full recalculation");
                        indicator.values = MovingAverage::calculate_ema(&candle_data.candles, indicator.period);
                    }
                }

                viewport_state.needs_redraw = true;
            }
        }

        viewport_state.loading = false;
    }

    // Load more recent data when scrolling right
    if end_idx > candle_data.candles.len().saturating_sub(interaction_config.lazy_load_threshold) && candle_data.candles.last().is_some() {
        viewport_state.loading = true;

        let load_start_time = candle_data.candles.last().unwrap().time;
        let interval_ms = chart_metadata.interval_ms();
        let load_end_time = load_start_time + (interaction_config.lazy_load_batch_size * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart_metadata.ticker_id,
            &chart_metadata.timeframe,
            load_start_time + 1, // Exclude the last candle we already have
            load_end_time,
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} recent candles", new_candles.len());

                // Store old length before extending
                let old_len = candle_data.candles.len();

                // Append new candles
                candle_data.candles.extend(new_candles);

                // INCREMENTAL: Only calculate MA for NEW candles
                println!("Recalculating indicators from index {} (append)", old_len);

                // Update indicators
                for indicator in indicator_state.indicators.iter_mut() {
                    if indicator.name.starts_with("SMA") {
                        indicator.calculate_sma_append(&candle_data.candles, old_len);
                    } else if indicator.name.starts_with("EMA") {
                        // EMA requires recursive calculation, must recalculate all
                        println!("Warning: EMA requires full recalculation");
                        indicator.values = MovingAverage::calculate_ema(&candle_data.candles, indicator.period);
                    }
                }

                viewport_state.needs_redraw = true;
            }
        }

        viewport_state.loading = false;
    }
}

/// Toggle volume pane visibility with 'V' key
pub fn toggle_volume_pane(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_state: ResMut<VolumeToggleState>,
    candle_data: Res<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut pane_manager: ResMut<PaneManager>,
    indicator_state: Res<IndicatorState>,
    interaction_config: Res<InteractionConfig>,
) {
    use crate::config;

    if keys.just_pressed(KeyCode::KeyV) {
        toggle_state.visible = !toggle_state.visible;

        if toggle_state.visible {
            // Add volume pane back (if not already present)
            let has_volume = pane_manager.panes.iter().any(|p| matches!(p.id, PaneId::Volume));

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

                println!("Volume pane shown");
            }
        } else {
            // Remove volume pane
            pane_manager.panes.retain(|p| !matches!(p.id, PaneId::Volume));

            // Give price pane 100% height
            if let Some(price_pane) = pane_manager.find_pane_mut(PaneId::Price) {
                price_pane.height_percent = config::FULL_PANE_HEIGHT;
            }

            println!("Volume pane hidden");
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
}

/// Toggle SMA indicator visibility with number keys
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
        // Toggle SMA-20 (first indicator)
        if let Some(sma) = indicator_state.indicators.get_mut(0) {
            sma.visible = !sma.visible;
            println!("SMA-{} {}", sma.period, if sma.visible { "shown" } else { "hidden" });
            toggled = true;
        }
    }

    if keys.just_pressed(KeyCode::Digit2) {
        // Toggle SMA-50 (second indicator)
        if let Some(sma) = indicator_state.indicators.get_mut(1) {
            sma.visible = !sma.visible;
            println!("SMA-{} {}", sma.period, if sma.visible { "shown" } else { "hidden" });
            toggled = true;
        }
    }

    if keys.just_pressed(KeyCode::Digit3) {
        // Toggle SMA-200 (third indicator)
        if let Some(sma) = indicator_state.indicators.get_mut(2) {
            sma.visible = !sma.visible;
            println!("SMA-{} {}", sma.period, if sma.visible { "shown" } else { "hidden" });
            toggled = true;
        }
    }

    // Toggle all SMAs with 'S' key
    if keys.just_pressed(KeyCode::KeyS) {
        indicator_state.toggle_all();
        let any_visible = indicator_state.indicators.iter().any(|ma| ma.visible);
        println!("All SMAs {}", if any_visible { "shown" } else { "hidden" });
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
