use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::CursorOptions;
use chrono::{DateTime, Utc};
use crate::types::*;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Format volume numbers with K/M/B suffixes
fn format_volume(value: f32) -> String {
    if value >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else {
        format!("{:.2}", value)
    }
}

// ============================================================================
// RENDERING SYSTEMS
// ============================================================================

pub fn render_candlesticks(
    mut commands: Commands,
    chart: Res<Chart>,
    query: Query<Entity, With<PriceElement>>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn all existing price elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Find the Price pane
    let price_pane = chart.panes.iter()
        .find(|p| matches!(p.id, PaneId::Price));

    if price_pane.is_none() {
        return;
    }
    let price_pane = price_pane.unwrap();

    // Get shared X-axis state
    let start = chart.visible_candle_start;
    let end = (start + chart.visible_candle_count).min(chart.candles.len());

    if start >= end {
        return;
    }

    for i in start..end {
        let candle = &chart.candles[i];

        // Calculate positions using ChartSpace::to_world() with shared X-axis params
        let wick_bottom = price_pane.space.to_world(
            i, candle.low as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let wick_top = price_pane.space.to_world(
            i, candle.high as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let body_open = price_pane.space.to_world(
            i, candle.open as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let body_close = price_pane.space.to_world(
            i, candle.close as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );

        let wick_center = Vec2::new(
            (wick_bottom.x + wick_top.x) / 2.0,
            (wick_bottom.y + wick_top.y) / 2.0,
        );
        let wick_height = (wick_top.y - wick_bottom.y).abs().max(1.0);

        // Spawn wick entity (thin line, Z=0)
        commands.spawn((
            Sprite {
                color: Color::srgb(0.5, 0.5, 0.5),
                custom_size: Some(Vec2::new(1.0, wick_height)),
                ..default()
            },
            Transform::from_translation(wick_center.extend(0.0)),
            CandlestickWick { candle_index: i },
            PriceElement,
            PaneId::Price,
        ));

        // Spawn body entity (rectangle, Z=1 above wick)
        let body_width = price_pane.space.candle_width_px * 0.7;
        let body_height = (body_close.y - body_open.y).abs().max(1.0);
        let body_center = Vec2::new(
            (body_open.x + body_close.x) / 2.0,
            (body_open.y + body_close.y) / 2.0,
        );

        let body_color = if candle.close >= candle.open {
            Color::srgb(0.0, 0.8, 0.2)  // Green
        } else {
            Color::srgb(0.9, 0.2, 0.2)  // Red
        };

        commands.spawn((
            Sprite {
                color: body_color,
                custom_size: Some(Vec2::new(body_width, body_height)),
                ..default()
            },
            Transform::from_translation(body_center.extend(1.0)),
            CandlestickBody { candle_index: i },
            PriceElement,
            PaneId::Price,
        ));
    }

    println!(
        "Rendered {} candles (indices {}-{})",
        end - start,
        start,
        end - 1
    );
}

pub fn render_volume_bars(
    mut commands: Commands,
    chart: Res<Chart>,
    query: Query<Entity, With<VolumeElement>>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn all existing volume bars
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Find the Volume pane
    let volume_pane = chart.panes.iter()
        .find(|p| matches!(p.id, PaneId::Volume));

    if volume_pane.is_none() {
        return;
    }
    let volume_pane = volume_pane.unwrap();

    // Get shared X-axis state
    let start = chart.visible_candle_start;
    let end = (start + chart.visible_candle_count).min(chart.candles.len());

    if start >= end {
        return;
    }

    for i in start..end {
        let candle = &chart.candles[i];

        // Calculate bottom (0) and top (volume) positions
        let bar_bottom = volume_pane.space.to_world(
            i, 0.0,
            chart.visible_candle_start, chart.visible_candle_count
        );
        let bar_top = volume_pane.space.to_world(
            i, candle.volume as f32,
            chart.visible_candle_start, chart.visible_candle_count
        );

        let bar_center = Vec2::new(
            (bar_bottom.x + bar_top.x) / 2.0,
            (bar_bottom.y + bar_top.y) / 2.0,
        );
        let bar_height = (bar_top.y - bar_bottom.y).abs().max(1.0);
        let bar_width = volume_pane.space.candle_width_px * 0.7;

        // Color based on candle direction
        let bar_color = if candle.close >= candle.open {
            Color::srgba(0.0, 0.8, 0.2, 0.6)  // Green with transparency
        } else {
            Color::srgba(0.9, 0.2, 0.2, 0.6)  // Red with transparency
        };

        commands.spawn((
            Sprite {
                    color: bar_color,
                    custom_size: Some(Vec2::new(bar_width, bar_height)),
                    ..default()
                },
            Transform::from_translation(bar_center.extend(0.0)),
            VolumeBar { candle_index: i },
            VolumeElement,
            PaneId::Volume,
        ));
    }

    println!(
        "Rendered {} volume bars (indices {}-{})",
        end - start,
        start,
        end - 1
    );
}

/// Initialize persistent crosshair entities (called once at startup from setup)
pub fn init_crosshair(
    commands: &mut Commands,
    chart: &Chart,
) {
    let mut horizontal_lines = Vec::new();
    let mut price_labels = Vec::new();

    // Calculate chart bounds
    if chart.panes.is_empty() {
        return;
    }

    let first_pane = &chart.panes[0];
    let last_pane = &chart.panes[chart.panes.len() - 1];
    let chart_top = first_pane.space.viewport.max.y;
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_right = first_pane.space.viewport.max.x;
    let total_height = chart_top - chart_bottom;

    // Dash pattern parameters
    const DASH_LENGTH: f32 = 8.0;
    const GAP_LENGTH: f32 = 4.0;
    const PATTERN_LENGTH: f32 = DASH_LENGTH + GAP_LENGTH;

    // Spawn vertical line segments (dashed pattern)
    let num_segments = (total_height / PATTERN_LENGTH).ceil() as usize;
    let mut vertical_line_segments = Vec::new();

    for i in 0..num_segments {
        let segment_y = chart_bottom + (i as f32 * PATTERN_LENGTH) + (DASH_LENGTH / 2.0);
        let segment_id = commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, 0.6),
                custom_size: Some(Vec2::new(1.0, DASH_LENGTH)),
                ..default()
            },
            Transform::from_translation(Vec3::new(0.0, segment_y, 3.0)),
            Visibility::Hidden,
            CrosshairElement,
        )).id();
        vertical_line_segments.push(segment_id);
    }

    // Spawn horizontal lines and price labels (one per pane)
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        // Horizontal line segments for this pane (dashed pattern)
        let pane_width = viewport.width();
        let num_h_segments = (pane_width / PATTERN_LENGTH).ceil() as usize;
        let mut h_segments = Vec::new();

        for i in 0..num_h_segments {
            let segment_x = viewport.min.x + (i as f32 * PATTERN_LENGTH) + (DASH_LENGTH / 2.0);
            let segment_id = commands.spawn((
                Sprite {
                    color: Color::srgba(1.0, 1.0, 1.0, 0.6),
                    custom_size: Some(Vec2::new(DASH_LENGTH, 1.0)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(segment_x, 0.0, 3.0)),
                Visibility::Hidden,
                CrosshairElement,
            )).id();
            h_segments.push(segment_id);
        }
        horizontal_lines.push((pane.id, h_segments));

        // Price label for this pane
        let label = commands.spawn((
            Text2d::new(""),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
            Anchor::CENTER_LEFT,
            Transform::from_translation(Vec3::new(chart_right + 50.0, 0.0, 4.0)),
            Visibility::Hidden,
            CrosshairElement,
        )).id();
        price_labels.push((pane.id, label));
    }

    // Spawn time label
    let time_label = commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.0)),
        Anchor::CENTER,
        Transform::from_translation(Vec3::new(0.0, chart_bottom - 40.0, 4.0)),
        Visibility::Hidden,
        CrosshairElement,
    )).id();

    // Spawn OHLCV info box
    let ohlcv_box = commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
        Anchor::TOP_LEFT,
        Transform::from_translation(Vec3::new(
            first_pane.space.viewport.min.x + 100.0,
            first_pane.space.viewport.max.y - 40.0,
            4.0
        )),
        Visibility::Hidden,
        CrosshairElement,
    )).id();

    // Insert the resource
    commands.insert_resource(CrosshairEntities {
        vertical_line_segments,
        horizontal_lines,
        price_labels,
        time_label,
        ohlcv_box,
    });

    println!("Initialized persistent crosshair entities (dashed pattern)");
}

pub fn render_grid_and_axes(
    mut commands: Commands,
    chart: Res<Chart>,
    grid: Res<ChartGrid>,
    axes: Res<ChartAxes>,
    query: Query<Entity, With<GridElement>>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn existing grid elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if !grid.show_grid || chart.panes.is_empty() {
        return;
    }

    // Calculate total chart bounds (from top of first pane to bottom of last pane)
    let first_pane = &chart.panes[0];
    let last_pane = &chart.panes[chart.panes.len() - 1];
    let chart_top = first_pane.space.viewport.max.y;
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_left = first_pane.space.viewport.min.x;
    let chart_right = first_pane.space.viewport.max.x;

    // ========== PER-PANE HORIZONTAL GRID LINES & Y-AXIS LABELS ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        // For volume panes, calculate render height ratio (before 12% padding)
        let render_height_ratio = match pane.pane_type {
            PaneType::Volume => {
                // visible_price_max = actual_max * 1.12
                // Grid lines should only go up to actual_max / visible_price_max = 1/1.12
                1.0 / 1.12
            },
            _ => 1.0
        };

        for i in 0..=grid.y_tick_count {
            let value_percent = i as f32 / grid.y_tick_count as f32;

            // For volume panes, don't render grid lines above the actual data range
            if value_percent > render_height_ratio {
                continue;
            }

            let value = pane.space.visible_price_min +
                value_percent * (pane.space.visible_price_max - pane.space.visible_price_min);

            let y = viewport.min.y + value_percent * viewport.height();

            // Draw horizontal line
            let line_center = Vec2::new((chart_left + chart_right) / 2.0, y);
            let line_width = chart_right - chart_left;

            commands.spawn((
                Sprite {
                        color: grid.grid_color,
                        custom_size: Some(Vec2::new(line_width, 1.0)),
                        ..default()
                    },
            Transform::from_translation(line_center.extend(-1.0)),
                GridElement,
            ));

            // Y-axis label on the right side
            if axes.show_y_labels {
                let label_x = chart_right + 50.0;

                // Format based on pane type
                let label_text = match pane.pane_type {
                    PaneType::Volume => format_volume(value / 1.12),
                    _ => format!("{:.2}", value)
                };

                commands.spawn((
                    Text2d::new(label_text),
                    TextFont {
                        font_size: axes.label_size,
                        ..default()
                    },
                    TextColor(axes.label_color),
                    Anchor::CENTER_LEFT,
                    Transform::from_translation(Vec3::new(label_x, y, 2.0)),
                    GridElement,
                ));
            }
        }
    }

    // ========== PANE BORDERS (Panel-style separation) ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;
        let border_color = Color::srgba(0.5, 0.5, 0.5, 0.6);
        let border_thickness = 2.0;

        // Top border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                viewport.center().x,
                viewport.max.y,
                0.4,
            )),
            GridElement,
        ));

        // Bottom border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                viewport.center().x,
                viewport.min.y,
                0.4,
            )),
            GridElement,
        ));

        // Left border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                viewport.min.x,
                viewport.center().y,
                0.4,
            )),
            GridElement,
        ));

        // Right border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                viewport.max.x,
                viewport.center().y,
                0.4,
            )),
            GridElement,
        ));
    }

    // ========== RESIZE GRIP (Horizontal lines in gaps) ==========
    for i in 0..chart.panes.len() - 1 {
        let pane_bottom = chart.panes[i].space.viewport.min.y;
        let next_pane_top = chart.panes[i + 1].space.viewport.max.y;
        let gap_center_y = (pane_bottom + next_pane_top) / 2.0;
        let gap_center_x = (chart_left + chart_right) / 2.0;

        let grip_width = 50.0;
        let line_height = 2.0;
        let line_spacing = 4.0;
        let grip_color = Color::srgba(0.6, 0.6, 0.6, 0.7);

        // Draw 2 horizontal lines
        for j in 0..2 {
            let offset = (j as f32 - 0.5) * (line_height + line_spacing);
            let line_y = gap_center_y + offset;

            commands.spawn((
                Sprite {
                    color: grip_color,
                    custom_size: Some(Vec2::new(grip_width, line_height)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(gap_center_x, line_y, 0.6)),
                GridElement,
            ));
        }
    }

    // ========== VERTICAL GRID LINES (Time - per pane) ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        for i in 0..=grid.x_tick_count {
            let candle_percent = i as f32 / grid.x_tick_count as f32;
            let candle_index = chart.visible_candle_start +
                (candle_percent * chart.visible_candle_count as f32) as usize;

            if candle_index >= chart.candles.len() {
                continue;
            }

            let x = chart_left + candle_percent * (chart_right - chart_left);

            // Draw vertical line within this pane only
            let line_center = Vec2::new(x, viewport.center().y);
            let line_height = viewport.height();

            commands.spawn((
                Sprite {
                        color: grid.grid_color,
                        custom_size: Some(Vec2::new(1.0, line_height)),
                        ..default()
                    },
            Transform::from_translation(line_center.extend(-1.0)),
                GridElement,
            ));
        }
    }

    // ========== X-AXIS TIME LABELS (at bottom of last pane) ==========
    if axes.show_x_labels {
        for i in 0..=grid.x_tick_count {
            let candle_percent = i as f32 / grid.x_tick_count as f32;
            let candle_index = chart.visible_candle_start +
                (candle_percent * chart.visible_candle_count as f32) as usize;

            if candle_index >= chart.candles.len() {
                continue;
            }

            let x = chart_left + candle_percent * (chart_right - chart_left);
            let candle = &chart.candles[candle_index];
            let label_y = chart_bottom - 40.0;

            // Format timestamp using chrono
            let datetime = DateTime::<Utc>::from_timestamp(candle.time / 1000, 0)
                .unwrap_or_default();
            let label_text = datetime.format("%m/%d %H:%M").to_string();

            commands.spawn((
                Text2d::new(label_text),
                TextFont {
                    font_size: axes.label_size,
                    ..default()
                },
                TextColor(axes.label_color),
                Anchor::CENTER,
                Transform::from_translation(Vec3::new(x, label_y, 2.0)),
                GridElement,
            ));
        }
    }
}

pub fn update_crosshair(
    crosshair_entities: Res<CrosshairEntities>,
    chart: Res<Chart>,
    crosshair: Res<Crosshair>,
    mut interaction: ResMut<InteractionState>,
    mut transforms: Query<&mut Transform>,
    mut visibilities: Query<&mut Visibility>,
    mut texts: Query<&mut Text>,
    mut cursor_options: Query<&mut CursorOptions, With<Window>>,
) {
    if !crosshair.enabled || chart.panes.is_empty() {
        // Hide all crosshair elements and show cursor
        for segment in &crosshair_entities.vertical_line_segments {
            if let Ok(mut vis) = visibilities.get_mut(*segment) {
                *vis = Visibility::Hidden;
            }
        }
        for (_, segments) in &crosshair_entities.horizontal_lines {
            for segment in segments {
                if let Ok(mut vis) = visibilities.get_mut(*segment) {
                    *vis = Visibility::Hidden;
                }
            }
        }
        for (_, entity) in &crosshair_entities.price_labels {
            if let Ok(mut vis) = visibilities.get_mut(*entity) {
                *vis = Visibility::Hidden;
            }
        }
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
            *vis = Visibility::Hidden;
        }
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
            *vis = Visibility::Hidden;
        }

        // Show cursor when crosshair disabled
        for mut opts in cursor_options.iter_mut() {
            opts.visible = true;
        }
        return;
    }

    // Calculate total chart bounds (from top of first pane to bottom of last pane)
    let first_pane = &chart.panes[0];
    let last_pane = &chart.panes[chart.panes.len() - 1];
    let chart_top = first_pane.space.viewport.max.y;
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_right = first_pane.space.viewport.max.x;

    // Check if mouse is within any pane
    let mouse_in_chart = chart.panes.iter().any(|pane| {
        pane.space.viewport.contains(interaction.mouse_pos)
    });

    // Show cursor if: outside chart, dragging, or resizing
    // Hide cursor only when: in chart AND not interacting
    let should_hide_cursor = mouse_in_chart
        && !interaction.dragging
        && interaction.hover_resize_gap.is_none()
        && interaction.resizing_gap.is_none();

    for mut opts in cursor_options.iter_mut() {
        opts.visible = !should_hide_cursor;
    }

    if !mouse_in_chart {
        // Hide all crosshair elements when mouse is outside chart
        for segment in &crosshair_entities.vertical_line_segments {
            if let Ok(mut vis) = visibilities.get_mut(*segment) {
                *vis = Visibility::Hidden;
            }
        }
        for (_, segments) in &crosshair_entities.horizontal_lines {
            for segment in segments {
                if let Ok(mut vis) = visibilities.get_mut(*segment) {
                    *vis = Visibility::Hidden;
                }
            }
        }
        for (_, entity) in &crosshair_entities.price_labels {
            if let Ok(mut vis) = visibilities.get_mut(*entity) {
                *vis = Visibility::Hidden;
            }
        }
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
            *vis = Visibility::Hidden;
        }
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let mouse_x = interaction.mouse_pos.x;
    let mouse_y = interaction.mouse_pos.y;

    // ========== OPTIMIZATION: Only update positions if mouse moved ==========
    let mouse_moved = interaction.mouse_pos != interaction.last_crosshair_mouse_pos;

    if mouse_moved {
        interaction.last_crosshair_mouse_pos = interaction.mouse_pos;

        // ========== UPDATE VERTICAL CROSSHAIR LINE SEGMENTS ==========
        for segment in &crosshair_entities.vertical_line_segments {
            if let Ok(mut transform) = transforms.get_mut(*segment) {
                transform.translation.x = mouse_x;
            }
            if let Ok(mut vis) = visibilities.get_mut(*segment) {
                *vis = Visibility::Visible;
            }
        }

        // ========== UPDATE PER-PANE HORIZONTAL LINES & LABELS ==========
        for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        // Find entities for this pane
        let h_line_segments = crosshair_entities.horizontal_lines.iter()
            .find(|(id, _)| *id == pane.id)
            .map(|(_, segments)| segments);
        let label_entity = crosshair_entities.price_labels.iter()
            .find(|(id, _)| *id == pane.id)
            .map(|(_, e)| *e);

        // Check if mouse Y is within this pane
        if mouse_y >= viewport.min.y && mouse_y <= viewport.max.y {
            // Update horizontal line segments position
            if let Some(segments) = h_line_segments {
                for segment in segments {
                    if let Ok(mut transform) = transforms.get_mut(*segment) {
                        transform.translation.y = mouse_y;
                    }
                    if let Ok(mut vis) = visibilities.get_mut(*segment) {
                        *vis = Visibility::Visible;
                    }
                }
            }

            // Update price label
            if crosshair.show_price_label {
                if let Some(entity) = label_entity {
                    let (_, value_at_cursor) = pane.space.from_world(
                        interaction.mouse_pos,
                        chart.visible_candle_start,
                        chart.visible_candle_count
                    );

                    if let Ok(mut text) = texts.get_mut(entity) {
                        text.0 = format!("{:.2}", value_at_cursor);
                    }
                    if let Ok(mut transform) = transforms.get_mut(entity) {
                        transform.translation.y = mouse_y;
                    }
                    if let Ok(mut vis) = visibilities.get_mut(entity) {
                        *vis = Visibility::Visible;
                    }
                }
            } else if let Some(entity) = label_entity {
                if let Ok(mut vis) = visibilities.get_mut(entity) {
                    *vis = Visibility::Hidden;
                }
            }
        } else {
            // Mouse not in this pane, hide its elements
            if let Some(segments) = h_line_segments {
                for segment in segments {
                    if let Ok(mut vis) = visibilities.get_mut(*segment) {
                        *vis = Visibility::Hidden;
                    }
                }
            }
            if let Some(entity) = label_entity {
                if let Ok(mut vis) = visibilities.get_mut(entity) {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
    } // End of mouse_moved check

    // ========== FIND CANDLE AT CURSOR ==========
    let (candle_index, _) = if let Some(pane) = chart.panes.first() {
        pane.space.from_world(interaction.mouse_pos, chart.visible_candle_start, chart.visible_candle_count)
    } else {
        return;
    };

    if candle_index >= chart.candles.len() {
        // Hide time label and OHLCV box if no valid candle
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
            *vis = Visibility::Hidden;
        }
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
            *vis = Visibility::Hidden;
        }
        interaction.last_crosshair_candle_index = None;
        return;
    }

    // ========== DEBOUNCE: Only update text if candle changed ==========
    let candle_changed = interaction.last_crosshair_candle_index != Some(candle_index);

    if candle_changed {
        interaction.last_crosshair_candle_index = Some(candle_index);
        let candle = &chart.candles[candle_index];

        // ========== UPDATE TIME LABEL (only when candle changes) ==========
        if crosshair.show_time_label {
            let datetime = DateTime::<Utc>::from_timestamp(candle.time / 1000, 0)
                .unwrap_or_default();
            let label_text = datetime.format("%m/%d %H:%M").to_string();

            if let Ok(mut text) = texts.get_mut(crosshair_entities.time_label) {
                text.0 = label_text;
            }
        }

        // ========== UPDATE OHLCV INFO BOX (only when candle changes) ==========
        if crosshair.show_ohlcv_box {
            let info_text = format!(
                "O: {:.2}  H: {:.2}  L: {:.2}  C: {:.2}\nVol: {:.2}",
                candle.open, candle.high, candle.low, candle.close, candle.volume
            );

            if let Ok(mut text) = texts.get_mut(crosshair_entities.ohlcv_box) {
                text.0 = info_text;
            }
        }
    }

    // ========== UPDATE POSITIONS & VISIBILITY (every frame) ==========
    // Time label position
    if crosshair.show_time_label {
        if let Ok(mut transform) = transforms.get_mut(crosshair_entities.time_label) {
            transform.translation.x = mouse_x;
        }
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
            *vis = Visibility::Visible;
        }
    } else {
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
            *vis = Visibility::Hidden;
        }
    }

    // OHLCV box visibility
    if crosshair.show_ohlcv_box {
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
            *vis = Visibility::Visible;
        }
    } else {
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
            *vis = Visibility::Hidden;
        }
    }
}

pub fn render_moving_averages(
    mut gizmos: Gizmos,
    chart: Res<Chart>,
) {
    // Find the Price pane (indicators overlay on price)
    let price_pane = chart.panes.iter()
        .find(|p| matches!(p.id, PaneId::Price));

    if price_pane.is_none() {
        return;
    }
    let price_pane = price_pane.unwrap();

    // Get shared X-axis state
    let start = chart.visible_candle_start;
    let end = (start + chart.visible_candle_count).min(chart.candles.len());

    if start >= end {
        return;
    }

    // Render each Moving Average using Gizmos for smooth continuous lines
    for ma in &chart.indicators {
        if !ma.visible || ma.values.is_empty() {
            continue;
        }

        // Draw continuous line connecting MA points
        for i in start..(end - 1) {
            // Need both current and next values to draw a line segment
            if let (Some(curr_value), Some(next_value)) = (ma.values[i], ma.values.get(i + 1).and_then(|v| *v)) {
                // Convert to world coordinates
                let curr_pos = price_pane.space.to_world(
                    i,
                    curr_value,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );

                let next_pos = price_pane.space.to_world(
                    i + 1,
                    next_value,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );

                // Draw line segment with Gizmos (no gaps or artifacts!)
                gizmos.line_2d(curr_pos, next_pos, ma.color);
            }
        }
    }
}
