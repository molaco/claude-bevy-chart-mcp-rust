use crate::cache::RenderCache;
use crate::focus::{FocusState, FocusTarget};
use crate::types::*;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::{CursorIcon, SystemCursorIcon};

// ============================================================================
// INTERACTION SYSTEMS
// ============================================================================

pub fn handle_mouse_input(
    mut chart: ResMut<Chart>,
    mut interaction: ResMut<InteractionState>,
    mut render_cache: ResMut<RenderCache>,
    config: Res<CandlestickLODConfig>,
    agg_state: Res<crate::aggregation::AggregationState>,
    zoom_limit_config: Res<ZoomLimitConfig>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut window_query: Query<(&mut Window, &mut CursorIcon)>,
) {
    let Ok((mut window, mut cursor_icon)) = window_query.single_mut() else {
        return;
    };

    // Update mouse position (convert to world space)
    // Bevy's cursor_position() has (0,0) at top-left with Y down
    // World space has (0,0) at center with Y up - need to flip Y
    if let Some(cursor_pos) = window.cursor_position() {
        // Check if cursor is within chart area (left 70%)
        let chart_width = window.width() * 0.7;
        if cursor_pos.x > chart_width {
            // Cursor is in chat UI area, don't process chart interactions
            return;
        }

        let window_size = Vec2::new(window.width(), window.height());
        interaction.mouse_pos = Vec2::new(
            cursor_pos.x - window_size.x / 2.0,
            window_size.y / 2.0 - cursor_pos.y, // Flip Y axis
        );
    }

    let mouse_y = interaction.mouse_pos.y;

    // ========== PANE RESIZE: Gap Detection ==========
    interaction.hover_resize_gap = None;
    for i in 0..chart.panes.len().saturating_sub(1) {
        let pane_bottom = chart.panes[i].space.viewport.min.y;
        let next_pane_top = chart.panes[i + 1].space.viewport.max.y;

        // Check if mouse is in the gap between panes
        if mouse_y <= pane_bottom && mouse_y >= next_pane_top {
            interaction.hover_resize_gap = Some(i);
            break;
        }
    }

    // Change cursor when hovering gap
    if interaction.hover_resize_gap.is_some() || interaction.resizing_gap.is_some() {
        *cursor_icon = CursorIcon::System(SystemCursorIcon::NsResize);
    } else {
        *cursor_icon = CursorIcon::default();
    }

    // ========== PANE RESIZE: Start/Stop ==========
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some(gap_idx) = interaction.hover_resize_gap {
            // Start resize
            interaction.resizing_gap = Some(gap_idx);
            interaction.drag_start_pos = interaction.mouse_pos;
            interaction.resize_start_heights =
                chart.panes.iter().map(|p| p.height_percent).collect();
        }
    }

    if mouse_button.just_released(MouseButton::Left) {
        if interaction.resizing_gap.is_some() {
            interaction.resizing_gap = None;
            *cursor_icon = CursorIcon::default();
        }
    }

    // ========== PANE RESIZE: Drag Logic ==========
    if let Some(gap_idx) = interaction.resizing_gap {
        let delta_y = interaction.mouse_pos.y - interaction.drag_start_pos.y;

        // Calculate available height (excluding gaps)
        const SEPARATOR_GAP: f32 = 24.0;
        let num_gaps = chart.panes.len() - 1;
        let available_height = chart.total_area.height() - (num_gaps as f32 * SEPARATOR_GAP);

        // Convert pixel movement to percentage change
        let delta_percent = -delta_y / available_height; // Negative because Y is flipped

        // Get original heights
        let orig_above = interaction.resize_start_heights[gap_idx];
        let orig_below = interaction.resize_start_heights[gap_idx + 1];

        // Calculate new heights with constraints
        let new_above = (orig_above + delta_percent).clamp(0.1, 0.9);
        let new_below = (orig_below - delta_percent).clamp(0.1, 0.9);

        // Check if both constraints are satisfied
        let total_change = (new_above - orig_above).abs() + (new_below - orig_below).abs();
        if total_change > 0.001 {
            chart.panes[gap_idx].height_percent = new_above;
            chart.panes[gap_idx + 1].height_percent = new_below;

            // Store values before mutable borrow
            let total_area = chart.total_area;
            let visible_candle_count = chart.visible_candle_count;

            // Recalculate layouts
            calculate_pane_layouts(&mut chart.panes, total_area, visible_candle_count);

            // Update Y-axis bounds
            update_pane_bounds(&mut chart, config.volume_y_axis_padding);

            // Invalidate render cache after pane resize
            render_cache.clear_all();

            chart.needs_redraw = true;
        }

        return; // Skip pan/zoom while resizing
    }

    // Check if mouse is in any pane
    let mouse_in_pane = chart
        .panes
        .iter()
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
        // Issue #8 fix: Reset accumulated pan delta when drag ends
        interaction.accumulated_pan_delta = 0.0;
    }

    if interaction.dragging && !chart.panes.is_empty() {
        let delta_x = interaction.mouse_pos.x - interaction.drag_start_pos.x;
        let candle_width_px = chart.panes[0].space.effective_candle_width_px;

        // Issue #8 fix: Accumulate fractional movement to avoid dead zones
        // Instead of truncating immediately, we keep track of fractional candles
        let candle_delta = -delta_x / candle_width_px;
        interaction.accumulated_pan_delta += candle_delta;

        // Only apply integer movement, keeping the fractional part
        let candles_moved = interaction.accumulated_pan_delta as i32;

        if candles_moved != 0 {
            // Subtract the integer part, keeping the fractional remainder
            interaction.accumulated_pan_delta -= candles_moved as f32;

            // Update shared X-axis state
            let new_start = (chart.visible_candle_start as i32 + candles_moved).max(0) as usize;
            let spacing = right_spacing_candles(chart.visible_candle_count);
            chart.visible_candle_start = new_start
                .min((chart.candles.len() + spacing).saturating_sub(chart.visible_candle_count));

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart, config.volume_y_axis_padding);

            // Recalculate cached values for all panes after pan
            let visible_candle_count = chart.visible_candle_count;
            let agg_level = agg_state.current_level;
            let aggregated_count = if agg_level != crate::aggregation::AggregationLevel::None {
                visible_candle_count / agg_level.ratio()
            } else {
                visible_candle_count
            };
            for pane in chart.panes.iter_mut() {
                pane.space.recalculate_cache(visible_candle_count, agg_level, aggregated_count);
            }

            // Invalidate render cache after pan completes
            render_cache.clear_all();

            chart.needs_redraw = true;
            interaction.drag_start_pos = interaction.mouse_pos;
        }
    }

    // Zoom: Mouse wheel
    for event in mouse_wheel.read() {
        if mouse_in_pane && !chart.panes.is_empty() {
            let zoom_factor = if event.y > 0.0 { 0.9 } else { 1.1 };

            // Calculate focus candle using first pane
            let (focus_candle, _) = chart.panes[0].space.from_world(
                interaction.mouse_pos,
                chart.visible_candle_start,
                chart.visible_candle_count,
            );

            // Update shared X-axis state (zoom)
            let old_count = chart.visible_candle_count;
            let max_zoom = zoom_limit_config.max_for_timeframe(&chart.timeframe) as f32;
            let new_count = ((old_count as f32 * zoom_factor)
                .clamp(zoom_limit_config.global_min as f32, max_zoom) as usize)
                .min(chart.candles.len());

            let focus_offset = focus_candle.saturating_sub(chart.visible_candle_start);
            let focus_percent = focus_offset as f32 / old_count as f32;

            let spacing = right_spacing_candles(new_count);
            chart.visible_candle_start = focus_candle
                .saturating_sub((new_count as f32 * focus_percent) as usize)
                .min((chart.candles.len() + spacing).saturating_sub(new_count));
            chart.visible_candle_count = new_count;

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart, config.volume_y_axis_padding);

            // Recalculate cached values for all panes after zoom
            let visible_candle_count = chart.visible_candle_count;
            let agg_level = agg_state.current_level;
            let aggregated_count = if agg_level != crate::aggregation::AggregationLevel::None {
                visible_candle_count / agg_level.ratio()
            } else {
                visible_candle_count
            };
            for pane in chart.panes.iter_mut() {
                pane.space.recalculate_cache(visible_candle_count, agg_level, aggregated_count);
            }

            // Invalidate render cache after zoom completes
            render_cache.clear_all();

            chart.needs_redraw = true;

            println!(
                "Zoomed: showing {} candles starting from {}",
                chart.visible_candle_count, chart.visible_candle_start
            );
        }
    }
}

/// Check if lazy loading is needed and queue data for next-frame application
/// Uses deferred updates pattern to prevent 1-frame rendering glitches
pub fn check_lazy_load(
    mut chart: ResMut<Chart>,
    mut deferred: ResMut<DeferredUpdates>,
    db: Res<ChartDatabase>,
) {
    // Skip if already loading or if there are pending updates waiting to be applied
    if chart.load_status != ChartLoadStatus::Ready || deferred.has_pending() {
        return;
    }

    let start_idx = chart.visible_candle_start;
    let end_idx = start_idx + chart.visible_candle_count;

    // Load more historical data when scrolling left
    if start_idx < 20 && chart.candles.first_key_value().is_some() {
        chart.load_status = ChartLoadStatus::Loading;

        let load_count = 100;
        let load_end_time = chart.candles.first_key_value().unwrap().1.time;

        // Calculate start time based on timeframe
        let interval_ms = match chart.timeframe.as_str() {
            "15m" => 15 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            "4h" => 4 * 60 * 60 * 1000,
            "1d" => 24 * 60 * 60 * 1000,
            _ => 60 * 60 * 1000,
        };
        let load_start_time = load_end_time - (load_count as i64 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time,
            load_end_time - 1, // Exclude the first candle we already have
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} historical candles (deferred)", new_candles.len());

                // Queue for next-frame application instead of immediate update
                deferred.candles_to_prepend = Some(new_candles);
                deferred.invalidate_aggregation = true;
                chart.load_status = ChartLoadStatus::PendingApply;
            } else {
                chart.load_status = ChartLoadStatus::Ready;
            }
        } else {
            chart.load_status = ChartLoadStatus::Ready;
        }
        return; // Don't check append in the same frame
    }

    // Load more recent data when scrolling right
    if end_idx > chart.candles.len().saturating_sub(20) && chart.candles.last_key_value().is_some() {
        chart.load_status = ChartLoadStatus::Loading;

        let load_start_time = chart.candles.last_key_value().unwrap().1.time;
        let interval_ms = match chart.timeframe.as_str() {
            "15m" => 15 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            "4h" => 4 * 60 * 60 * 1000,
            "1d" => 24 * 60 * 60 * 1000,
            _ => 60 * 60 * 1000,
        };
        let load_end_time = load_start_time + (100 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time + 1, // Exclude the last candle we already have
            load_end_time,
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} recent candles (deferred)", new_candles.len());

                // Queue for next-frame application instead of immediate update
                deferred.candles_to_append = Some(new_candles);
                deferred.invalidate_aggregation = true;
                chart.load_status = ChartLoadStatus::PendingApply;
            } else {
                chart.load_status = ChartLoadStatus::Ready;
            }
        } else {
            chart.load_status = ChartLoadStatus::Ready;
        }
    }
}

/// Apply deferred updates at the start of the frame
/// This ensures all data changes happen before rendering systems run
pub fn apply_deferred_updates(
    mut chart: ResMut<Chart>,
    mut deferred: ResMut<DeferredUpdates>,
    mut agg_cache: ResMut<crate::aggregation::AggregationCache>,
    mut agg_state: ResMut<crate::aggregation::AggregationState>,
) {
    if !deferred.has_pending() {
        return;
    }

    // Apply prepended candles (historical data)
    if let Some(new_candles) = deferred.candles_to_prepend.take() {
        let new_len = new_candles.len();
        println!("Applying {} prepended candles", new_len);

        // Insert new candles into BTreeMap (automatically sorted by key)
        for candle in new_candles {
            chart.candles.insert(candle.time, candle);
        }

        // Adjust visible_start to maintain view
        chart.visible_candle_start += new_len;

        // INCREMENTAL: Only calculate MA for NEW candles
        // Use index-based iteration to avoid borrow issues
        // Convert BTreeMap values to Vec for MA calculation
        let candles_vec: Vec<Candle> = chart.candles.values().cloned().collect();
        for i in 0..chart.indicators.len() {
            let indicator = &chart.indicators[i];
            if indicator.name.starts_with("SMA") {
                let period = indicator.period;
                let new_values = MovingAverage::calculate_sma(&candles_vec, period);
                chart.indicators[i].values = new_values;
            } else if indicator.name.starts_with("EMA") {
                let period = indicator.period;
                let new_values = MovingAverage::calculate_ema(&candles_vec, period);
                chart.indicators[i].values = new_values;
            }
        }
    }

    // Apply appended candles (recent data)
    if let Some(new_candles) = deferred.candles_to_append.take() {
        let _old_len = chart.candles.len();
        println!("Applying {} appended candles", new_candles.len());

        // Insert new candles into BTreeMap (automatically sorted by key)
        for candle in new_candles {
            chart.candles.insert(candle.time, candle);
        }

        // INCREMENTAL: Calculate new MA values
        // Collect calculation results first to avoid borrow conflicts
        // Convert BTreeMap values to Vec for MA calculation
        let candles_vec: Vec<Candle> = chart.candles.values().cloned().collect();
        let indicator_updates: Vec<(usize, Vec<Option<f32>>)> = chart
            .indicators
            .iter()
            .enumerate()
            .filter_map(|(i, indicator)| {
                if indicator.name.starts_with("SMA") || indicator.name.starts_with("EMA") {
                    let new_values = if indicator.name.starts_with("SMA") {
                        MovingAverage::calculate_sma(&candles_vec, indicator.period)
                    } else {
                        MovingAverage::calculate_ema(&candles_vec, indicator.period)
                    };
                    Some((i, new_values))
                } else {
                    None
                }
            })
            .collect();

        // Apply updates
        for (i, values) in indicator_updates {
            chart.indicators[i].values = values;
        }
    }

    // Invalidate aggregation cache if needed
    if deferred.invalidate_aggregation {
        agg_cache.clear_timeframe(&chart.timeframe);
        deferred.invalidate_aggregation = false;
    }

    // Reset aggregation state if needed
    if deferred.reset_aggregation_state {
        agg_state.current_level = crate::aggregation::AggregationLevel::None;
        agg_state.previous_level = crate::aggregation::AggregationLevel::None;
        agg_state.level_change_cooldown = None;
        deferred.reset_aggregation_state = false;
    }

    // Mark chart for redraw and set status to ready
    chart.needs_redraw = true;
    chart.load_status = ChartLoadStatus::Ready;
    chart.loading = false; // Keep legacy flag in sync
}

/// Toggle volume pane visibility with 'V' key
pub fn toggle_volume_pane(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<FocusState>,
    config: Res<CandlestickLODConfig>,
    mut toggle_state: ResMut<VolumeToggleState>,
    mut chart: ResMut<Chart>,
) {
    // Only process if chart has focus
    if focus.current != FocusTarget::Chart {
        return;
    }

    if keys.just_pressed(KeyCode::KeyV) {
        toggle_state.visible = !toggle_state.visible;

        if toggle_state.visible {
            // Add volume pane back (if not already present)
            let has_volume = chart.panes.iter().any(|p| matches!(p.id, PaneId::Volume));

            if !has_volume {
                // Insert volume pane after price pane
                let volume_pane = Pane::new(
                    PaneId::Volume,
                    PaneType::Volume,
                    0.3, // 30% height
                    Rect::default(),
                    chart.visible_candle_count,
                );
                chart.panes.push(volume_pane);

                // Adjust price pane height to 70%
                if let Some(price_pane) = chart
                    .panes
                    .iter_mut()
                    .find(|p| matches!(p.id, PaneId::Price))
                {
                    price_pane.height_percent = 0.7;
                }

                println!("Volume pane shown");
            }
        } else {
            // Remove volume pane
            chart.panes.retain(|p| !matches!(p.id, PaneId::Volume));

            // Give price pane 100% height
            if let Some(price_pane) = chart
                .panes
                .iter_mut()
                .find(|p| matches!(p.id, PaneId::Price))
            {
                price_pane.height_percent = 1.0;
            }

            println!("Volume pane hidden");
        }

        // Recalculate pane layouts
        let total_area = chart.total_area;
        let visible_candle_count = chart.visible_candle_count;
        calculate_pane_layouts(&mut chart.panes, total_area, visible_candle_count);

        // Update Y-axis bounds for all panes
        update_pane_bounds(&mut chart, config.volume_y_axis_padding);

        // Trigger redraw
        chart.needs_redraw = true;
    }
}

/// Toggle SMA indicator visibility with number keys
/// Keys: 1 = SMA-20, 2 = SMA-50, 3 = SMA-200, S = Toggle all SMAs
pub fn toggle_sma_indicators(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<FocusState>,
    config: Res<CandlestickLODConfig>,
    mut chart: ResMut<Chart>,
) {
    // Only process if chart has focus
    if focus.current != FocusTarget::Chart {
        return;
    }

    let mut toggled = false;

    // Toggle individual SMAs with number keys
    if keys.just_pressed(KeyCode::Digit1) {
        // Toggle SMA-20 (first indicator)
        if let Some(sma) = chart.indicators.get_mut(0) {
            sma.visible = !sma.visible;
            println!(
                "SMA-{} {}",
                sma.period,
                if sma.visible { "shown" } else { "hidden" }
            );
            toggled = true;
        }
    }

    if keys.just_pressed(KeyCode::Digit2) {
        // Toggle SMA-50 (second indicator)
        if let Some(sma) = chart.indicators.get_mut(1) {
            sma.visible = !sma.visible;
            println!(
                "SMA-{} {}",
                sma.period,
                if sma.visible { "shown" } else { "hidden" }
            );
            toggled = true;
        }
    }

    if keys.just_pressed(KeyCode::Digit3) {
        // Toggle SMA-200 (third indicator)
        if let Some(sma) = chart.indicators.get_mut(2) {
            sma.visible = !sma.visible;
            println!(
                "SMA-{} {}",
                sma.period,
                if sma.visible { "shown" } else { "hidden" }
            );
            toggled = true;
        }
    }

    // Toggle all SMAs with 'S' key
    if keys.just_pressed(KeyCode::KeyS) {
        // Check if any SMA is visible
        let any_visible = chart.indicators.iter().any(|ma| ma.visible);

        // Toggle all to opposite state
        let new_state = !any_visible;
        for ma in chart.indicators.iter_mut() {
            ma.visible = new_state;
        }

        println!("All SMAs {}", if new_state { "shown" } else { "hidden" });
        toggled = true;
    }

    // If any toggle occurred, update bounds and trigger redraw
    if toggled {
        // Update Y-axis bounds (will respect new visibility state)
        update_pane_bounds(&mut chart, config.volume_y_axis_padding);

        // Trigger redraw
        chart.needs_redraw = true;
    }
}
