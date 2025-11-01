use bevy::prelude::*;
use chrono::{DateTime, Utc};
use crate::types::*;

// ============================================================================
// RENDERING SYSTEMS
// ============================================================================

pub fn render_candlesticks(
    mut commands: Commands,
    chart: Res<Chart>,
    query: Query<Entity, With<ChartElement>>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Despawn all existing chart elements
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
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgb(0.5, 0.5, 0.5),
                    custom_size: Some(Vec2::new(1.0, wick_height)),
                    ..default()
                },
                transform: Transform::from_translation(wick_center.extend(0.0)),
                ..default()
            },
            CandlestickWick { candle_index: i },
            ChartElement,
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
            SpriteBundle {
                sprite: Sprite {
                    color: body_color,
                    custom_size: Some(Vec2::new(body_width, body_height)),
                    ..default()
                },
                transform: Transform::from_translation(body_center.extend(1.0)),
                ..default()
            },
            CandlestickBody { candle_index: i },
            ChartElement,
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
    query: Query<Entity, (With<VolumeBar>, With<ChartElement>)>,
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
            SpriteBundle {
                sprite: Sprite {
                    color: bar_color,
                    custom_size: Some(Vec2::new(bar_width, bar_height)),
                    ..default()
                },
                transform: Transform::from_translation(bar_center.extend(0.0)),
                ..default()
            },
            VolumeBar { candle_index: i },
            ChartElement,
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

        for i in 0..=grid.y_tick_count {
            let value_percent = i as f32 / grid.y_tick_count as f32;
            let value = pane.space.visible_price_min +
                value_percent * (pane.space.visible_price_max - pane.space.visible_price_min);

            let y = viewport.min.y + value_percent * viewport.height();

            // Draw horizontal line
            let line_center = Vec2::new((chart_left + chart_right) / 2.0, y);
            let line_width = chart_right - chart_left;

            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: grid.grid_color,
                        custom_size: Some(Vec2::new(line_width, 1.0)),
                        ..default()
                    },
                    transform: Transform::from_translation(line_center.extend(-1.0)),
                    ..default()
                },
                GridElement,
            ));

            // Y-axis label on the right side
            if axes.show_y_labels {
                let label_x = chart_right + 50.0;
                let label_text = format!("{:.2}", value);

                commands.spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            label_text,
                            TextStyle {
                                font_size: axes.label_size,
                                color: axes.label_color,
                                ..default()
                            },
                        ),
                        transform: Transform::from_translation(Vec3::new(label_x, y, 2.0)),
                        text_anchor: bevy::sprite::Anchor::CenterLeft,
                        ..default()
                    },
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
            SpriteBundle {
                sprite: Sprite {
                    color: border_color,
                    custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(
                    viewport.center().x,
                    viewport.max.y,
                    0.4,
                )),
                ..default()
            },
            GridElement,
        ));

        // Bottom border
        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: border_color,
                    custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(
                    viewport.center().x,
                    viewport.min.y,
                    0.4,
                )),
                ..default()
            },
            GridElement,
        ));

        // Left border
        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: border_color,
                    custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(
                    viewport.min.x,
                    viewport.center().y,
                    0.4,
                )),
                ..default()
            },
            GridElement,
        ));

        // Right border
        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: border_color,
                    custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(
                    viewport.max.x,
                    viewport.center().y,
                    0.4,
                )),
                ..default()
            },
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

        // Draw 4 horizontal lines
        for j in 0..4 {
            let offset = (j as f32 - 1.5) * (line_height + line_spacing);
            let line_y = gap_center_y + offset;

            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: grip_color,
                        custom_size: Some(Vec2::new(grip_width, line_height)),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(gap_center_x, line_y, 0.6)),
                    ..default()
                },
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
                SpriteBundle {
                    sprite: Sprite {
                        color: grid.grid_color,
                        custom_size: Some(Vec2::new(1.0, line_height)),
                        ..default()
                    },
                    transform: Transform::from_translation(line_center.extend(-1.0)),
                    ..default()
                },
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
                Text2dBundle {
                    text: Text::from_section(
                        label_text,
                        TextStyle {
                            font_size: axes.label_size,
                            color: axes.label_color,
                            ..default()
                        },
                    ),
                    transform: Transform::from_translation(Vec3::new(x, label_y, 2.0)),
                    text_anchor: bevy::sprite::Anchor::Center,
                    ..default()
                },
                GridElement,
            ));
        }
    }
}

pub fn render_crosshair(
    mut commands: Commands,
    chart: Res<Chart>,
    crosshair: Res<Crosshair>,
    interaction: Res<InteractionState>,
    query: Query<Entity, With<CrosshairElement>>,
) {
    // Despawn existing crosshair elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if !crosshair.enabled || chart.panes.is_empty() {
        return;
    }

    // Calculate total chart bounds (from top of first pane to bottom of last pane)
    let first_pane = &chart.panes[0];
    let last_pane = &chart.panes[chart.panes.len() - 1];
    let chart_top = first_pane.space.viewport.max.y;
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_left = first_pane.space.viewport.min.x;
    let chart_right = first_pane.space.viewport.max.x;

    // Check if mouse is within any pane
    let mouse_in_chart = chart.panes.iter().any(|pane| {
        pane.space.viewport.contains(interaction.mouse_pos)
    });

    if !mouse_in_chart {
        return;
    }

    let mouse_x = interaction.mouse_pos.x;
    let mouse_y = interaction.mouse_pos.y;

    // ========== VERTICAL CROSSHAIR LINE (spans all panes) ==========
    let total_height = chart_top - chart_bottom;
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: crosshair.line_color,
                custom_size: Some(Vec2::new(1.0, total_height)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(
                mouse_x,
                (chart_top + chart_bottom) / 2.0,
                3.0,
            )),
            ..default()
        },
        CrosshairElement,
    ));

    // ========== PER-PANE HORIZONTAL CROSSHAIR LINES & VALUE LABELS ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        // Check if mouse Y is within this pane
        if mouse_y >= viewport.min.y && mouse_y <= viewport.max.y {
            // Draw horizontal line within this pane
            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: crosshair.line_color,
                        custom_size: Some(Vec2::new(viewport.width(), 1.0)),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(
                        viewport.center().x,
                        mouse_y,
                        3.0,
                    )),
                    ..default()
                },
                CrosshairElement,
            ));

            // Show value label for this pane
            if crosshair.show_price_label {
                let (_, value_at_cursor) = pane.space.from_world(
                    interaction.mouse_pos,
                    chart.visible_candle_start,
                    chart.visible_candle_count
                );
                let label_text = format!("{:.2}", value_at_cursor);
                let label_x = chart_right + 50.0;

                commands.spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            label_text,
                            TextStyle {
                                font_size: 14.0,
                                color: Color::srgb(1.0, 1.0, 0.0), // Yellow for crosshair
                                ..default()
                            },
                        ),
                        transform: Transform::from_translation(Vec3::new(label_x, mouse_y, 4.0)),
                        text_anchor: bevy::sprite::Anchor::CenterLeft,
                        ..default()
                    },
                    CrosshairElement,
                ));
            }
        }
    }

    // ========== FIND CANDLE AT CURSOR ==========
    let (candle_index, _) = if let Some(pane) = chart.panes.first() {
        pane.space.from_world(interaction.mouse_pos, chart.visible_candle_start, chart.visible_candle_count)
    } else {
        return;
    };

    if candle_index >= chart.candles.len() {
        return;
    }

    let candle = &chart.candles[candle_index];

    // ========== TIME LABEL AT CROSSHAIR ==========
    if crosshair.show_time_label {
        let datetime = DateTime::<Utc>::from_timestamp(candle.time / 1000, 0)
            .unwrap_or_default();
        let label_text = datetime.format("%m/%d %H:%M").to_string();
        let label_y = chart_bottom - 40.0;

        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    label_text,
                    TextStyle {
                        font_size: 14.0,
                        color: Color::srgb(1.0, 1.0, 0.0), // Yellow for crosshair time
                        ..default()
                    },
                ),
                transform: Transform::from_translation(Vec3::new(mouse_x, label_y, 4.0)),
                text_anchor: bevy::sprite::Anchor::Center,
                ..default()
            },
            CrosshairElement,
        ));
    }

    // ========== OHLCV INFO BOX (at top of Price pane) ==========
    if crosshair.show_ohlcv_box {
        let info_text = format!(
            "O: {:.2}  H: {:.2}  L: {:.2}  C: {:.2}\nVol: {:.2}",
            candle.open, candle.high, candle.low, candle.close, candle.volume
        );

        // Find Price pane and position box at its top-left
        if let Some(price_pane) = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price)) {
            let info_x = price_pane.space.viewport.min.x + 100.0;
            let info_y = price_pane.space.viewport.max.y - 40.0;

            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        info_text,
                        TextStyle {
                            font_size: 16.0,
                            color: Color::srgb(1.0, 1.0, 1.0),
                            ..default()
                        },
                    ),
                    transform: Transform::from_translation(Vec3::new(info_x, info_y, 4.0)),
                    text_anchor: bevy::sprite::Anchor::TopLeft,
                    ..default()
                },
                CrosshairElement,
            ));
        }
    }
}
