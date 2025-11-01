use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use crate::types::*;

// ============================================================================
// INTERACTION SYSTEMS
// ============================================================================

pub fn handle_mouse_input(
    mut chart: ResMut<Chart>,
    mut interaction: ResMut<InteractionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: EventReader<MouseWheel>,
    window_query: Query<&Window>,
) {
    let Ok(window) = window_query.get_single() else {
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

    // Check if mouse is in any pane
    let mouse_in_pane = chart.panes.iter()
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

    if interaction.dragging && !chart.panes.is_empty() {
        let delta_x = interaction.mouse_pos.x - interaction.drag_start_pos.x;
        let candle_width_px = chart.panes[0].space.candle_width_px;
        let candles_moved = -(delta_x / candle_width_px) as i32;

        if candles_moved != 0 {
            // Update shared X-axis state
            let new_start = (chart.visible_candle_start as i32 + candles_moved).max(0) as usize;
            chart.visible_candle_start = new_start.min(
                chart.candles.len().saturating_sub(chart.visible_candle_count)
            );

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart);

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
                chart.visible_candle_count
            );

            // Update shared X-axis state (zoom)
            let old_count = chart.visible_candle_count;
            let new_count = ((old_count as f32 * zoom_factor).clamp(10.0, 10000.0) as usize)
                .min(chart.candles.len());

            let focus_offset = focus_candle.saturating_sub(chart.visible_candle_start);
            let focus_percent = focus_offset as f32 / old_count as f32;

            chart.visible_candle_start = focus_candle
                .saturating_sub((new_count as f32 * focus_percent) as usize)
                .min(chart.candles.len().saturating_sub(new_count));
            chart.visible_candle_count = new_count;

            // Update Y-axis bounds for all panes
            update_pane_bounds(&mut chart);

            chart.needs_redraw = true;

            println!(
                "Zoomed: showing {} candles starting from {}",
                chart.visible_candle_count,
                chart.visible_candle_start
            );
        }
    }
}

pub fn check_lazy_load(
    mut chart: ResMut<Chart>,
    db: Res<ChartDatabase>,
) {
    if chart.loading {
        return;  // Already loading
    }

    let start_idx = chart.visible_candle_start;
    let end_idx = start_idx + chart.visible_candle_count;

    // Load more historical data when scrolling left
    if start_idx < 20 && chart.candles.first().is_some() {
        chart.loading = true;

        let load_count = 100;
        let load_end_time = chart.candles.first().unwrap().time;

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
                println!("Lazy loaded {} historical candles", new_candles.len());

                // Prepend new candles
                let new_len = new_candles.len();
                let mut combined = new_candles;
                combined.append(&mut chart.candles);
                chart.candles = combined;

                // Adjust visible_start to maintain view
                chart.visible_candle_start += new_len;

                chart.needs_redraw = true;
            }
        }

        chart.loading = false;
    }

    // Load more recent data when scrolling right
    if end_idx > chart.candles.len().saturating_sub(20) && chart.candles.last().is_some() {
        chart.loading = true;

        let load_start_time = chart.candles.last().unwrap().time;
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
                println!("Lazy loaded {} recent candles", new_candles.len());
                chart.candles.extend(new_candles);
                chart.needs_redraw = true;
            }
        }

        chart.loading = false;
    }
}
