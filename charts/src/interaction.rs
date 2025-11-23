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
            let visible_candle_count = chart.visible_candle_count();

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
        interaction.accumulated_pan_delta = 0;
    }

    // Phase 3: Time-based panning
    if interaction.dragging && !chart.panes.is_empty() {
        let delta_x = interaction.mouse_pos.x - interaction.drag_start_pos.x;
        let viewport_width = chart.panes[0].space.viewport.width();

        // Convert pixel delta to time delta
        let visible_duration = chart.visible_time_end - chart.visible_time_start;
        let time_delta = (-delta_x / viewport_width) * visible_duration as f32;

        // Accumulate fractional time movement
        interaction.accumulated_pan_delta += time_delta as i64;

        // Get timeframe interval for snapping
        let interval_ms = chart.timeframe_interval_ms();

        // Apply movement with snapping to timeframe intervals
        if interaction.accumulated_pan_delta.abs() >= interval_ms {
            let snapped_delta = (interaction.accumulated_pan_delta / interval_ms) * interval_ms;
            interaction.accumulated_pan_delta -= snapped_delta;

            // Calculate bounds
            let earliest = chart.candles.keys().next().copied().unwrap_or(0);
            let latest = chart.candles.keys().next_back().copied().unwrap_or(0);

            // Apply time shift with bounds checking
            let new_start = (chart.visible_time_start + snapped_delta)
                .max(earliest)
                .min(latest - visible_duration / 2);
            let new_end = new_start + visible_duration;

            chart.visible_time_start = new_start;
            chart.visible_time_end = new_end;

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart, config.volume_y_axis_padding);

            // Recalculate cached values for all panes after pan
            let visible_candle_count = chart.visible_candle_count();
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
        }

        interaction.drag_start_pos = interaction.mouse_pos;
    }

    // Phase 3: Time-based zoom (mouse wheel)
    for event in mouse_wheel.read() {
        if mouse_in_pane && !chart.panes.is_empty() {
            let zoom_factor = if event.y > 0.0 { 0.9 } else { 1.1 };

            // Get timestamp at mouse position (zoom focus point)
            let (focus_time, _) = chart.panes[0].space.from_world(
                interaction.mouse_pos,
                chart.visible_time_start,
                chart.visible_time_end,
            );

            // Calculate new duration
            let old_duration = chart.visible_time_end - chart.visible_time_start;
            let new_duration = (old_duration as f64 * zoom_factor as f64) as i64;

            // Clamp duration to min/max
            let interval_ms = chart.timeframe_interval_ms();
            let min_duration = interval_ms * zoom_limit_config.global_min as i64;
            let max_duration = interval_ms * zoom_limit_config.max_for_timeframe(&chart.timeframe) as i64;
            let clamped_duration = new_duration.clamp(min_duration, max_duration);

            // Calculate focus percentage (where in the visible range is the mouse?)
            let focus_percent = (focus_time - chart.visible_time_start) as f64
                / old_duration as f64;

            // Calculate new start/end maintaining focus point
            let new_start = focus_time - (clamped_duration as f64 * focus_percent) as i64;
            let new_end = new_start + clamped_duration;

            // Apply bounds
            let earliest = chart.candles.keys().next().copied().unwrap_or(0);
            let latest = chart.candles.keys().next_back().copied().unwrap_or(0);

            chart.visible_time_start = new_start.max(earliest);
            chart.visible_time_end = new_end.min(latest + interval_ms * 10); // Allow some right space

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart, config.volume_y_axis_padding);

            // Recalculate cached values for all panes after zoom
            let visible_candle_count = chart.visible_candle_count();
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
                "Zoomed: showing {} to {} ({} candles)",
                chart.visible_time_start,
                chart.visible_time_end,
                chart.visible_candle_count()
            );
        }
    }
}

/// Check if lazy loading is needed and queue data for next-frame application
/// Uses deferred updates pattern to prevent 1-frame rendering glitches
/// Phase 6: Time-based lazy loading with buffer duration
pub fn check_lazy_load(
    mut chart: ResMut<Chart>,
    mut deferred: ResMut<DeferredUpdates>,
    db: Res<ChartDatabase>,
) {
    // Skip if already loading or if there are pending updates waiting to be applied
    if chart.load_status != ChartLoadStatus::Ready || deferred.has_pending() {
        return;
    }

    let interval_ms = chart.timeframe_interval_ms();
    let buffer_duration = interval_ms * 20; // 20 candles worth of buffer

    // Get data boundaries
    let earliest_data = chart.candles.keys().next().copied();
    let latest_data = chart.candles.keys().next_back().copied();

    if earliest_data.is_none() || latest_data.is_none() {
        return;
    }
    let earliest_data = earliest_data.unwrap();
    let latest_data = latest_data.unwrap();

    // Load historical data when approaching left edge
    if chart.visible_time_start < earliest_data + buffer_duration {
        chart.load_status = ChartLoadStatus::Loading;

        let load_count = 100;
        let load_end_time = earliest_data;
        let load_start_time = load_end_time - (load_count as i64 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time,
            load_end_time - 1, // Exclude the first candle we already have
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} historical candles", new_candles.len());

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

    // Load recent data when approaching right edge
    if chart.visible_time_end > latest_data - buffer_duration {
        chart.load_status = ChartLoadStatus::Loading;

        let load_start_time = latest_data;
        let load_end_time = load_start_time + (100 * interval_ms);

        if let Ok(new_candles) = db.load_candles(
            chart.ticker_id,
            &chart.timeframe,
            load_start_time + 1, // Exclude the last candle we already have
            load_end_time,
        ) {
            if !new_candles.is_empty() {
                println!("Lazy loaded {} recent candles", new_candles.len());

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
///
/// Phase 6 Key Insight: With time-based visible range, prepending/appending candles
/// requires NO adjustment to visible_time_start/visible_time_end!
/// - Old (index-based): After prepending 100 candles, must adjust visible_candle_start += 100
/// - New (time-based): visible_time_start and visible_time_end stay the same
///   The view shows the same time range, now with more data available
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
        println!("Applying {} prepended candles", new_candles.len());

        // Insert into BTreeMap (auto-sorted by timestamp)
        for candle in new_candles {
            chart.candles.insert(candle.time, candle);
        }

        // NO adjustment needed for visible_time_start/end!
        // Time-based view automatically shows the same time range
        // The view stays at the same timestamps, just with more data available

        // Recalculate indicators with full dataset
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
        println!("Applying {} appended candles", new_candles.len());

        // Insert into BTreeMap (auto-sorted by timestamp)
        for candle in new_candles {
            chart.candles.insert(candle.time, candle);
        }

        // NO adjustment needed - time range stays the same

        // Recalculate indicators with full dataset
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
                    chart.visible_candle_count(),
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
        let visible_candle_count = chart.visible_candle_count();
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
