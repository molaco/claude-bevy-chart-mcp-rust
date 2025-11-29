use crate::config::{ChartDimensions, ChartTheme, ZLayerConfig};
use crate::types::*;
use bevy::prelude::*;
use bevy::window::CursorOptions;
use chrono::{DateTime, Utc};

// ============================================================================
// RENDERING SYSTEMS
// ============================================================================

pub fn render_volume_bars(
    mut commands: Commands,
    viewport: Res<ViewportState>,
    pane_manager: Res<PaneManager>,
    candle_data: Res<CandleData>,
    theme: Res<ChartTheme>,
    dimensions: Res<ChartDimensions>,
    z_layers: Res<ZLayerConfig>,
    query: Query<Entity, With<VolumeElement>>,
) {
    if !viewport.needs_redraw {
        return;
    }

    // Despawn all existing volume bars
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Find the Volume pane
    let volume_pane = match pane_manager.find_pane(PaneId::Volume) {
        Some(pane) => pane,
        None => return,
    };

    // Get shared X-axis state
    let start = viewport.visible_candle_start;
    let end = (start + viewport.visible_candle_count).min(candle_data.candles.len());

    if start >= end {
        return;
    }

    for i in start..end {
        let candle = &candle_data.candles[i];

        // Calculate bottom (0) and top (volume) positions
        let bar_bottom = volume_pane.space.to_world(
            i,
            0.0,
            viewport.visible_candle_start,
            viewport.visible_candle_count,
        );
        let bar_top = volume_pane.space.to_world(
            i,
            candle.volume as f32,
            viewport.visible_candle_start,
            viewport.visible_candle_count,
        );

        let bar_center = Vec2::new(
            (bar_bottom.x + bar_top.x) / 2.0,
            (bar_bottom.y + bar_top.y) / 2.0,
        );
        let bar_height = (bar_top.y - bar_bottom.y).abs().max(dimensions.min_element_height);
        let bar_width = volume_pane.space.candle_width_px * dimensions.body_width_ratio;

        // Color based on candle direction (using theme colors)
        let bar_color = if candle.close >= candle.open {
            theme.bull_volume
        } else {
            theme.bear_volume
        };

        commands.spawn((
            Sprite {
                color: bar_color,
                custom_size: Some(Vec2::new(bar_width, bar_height)),
                ..default()
            },
            Transform::from_translation(bar_center.extend(z_layers.volume)),
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
/// Note: This is called from setup before config resources are inserted, so we use constants directly
pub fn init_crosshair(commands: &mut Commands, pane_manager: &PaneManager) {
    use crate::config;

    let mut horizontal_lines = Vec::new();
    let mut price_labels = Vec::new();

    // Calculate chart bounds
    if pane_manager.panes.is_empty() {
        return;
    }

    let first_pane = &pane_manager.panes[0];
    let last_pane = &pane_manager.panes[pane_manager.panes.len() - 1];
    let chart_top = first_pane.space.viewport.max.y;
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_right = first_pane.space.viewport.max.x;
    let total_height = chart_top - chart_bottom;

    // Dash pattern parameters from config
    let dash_length = config::CROSSHAIR_DASH_LENGTH;
    let gap_length = config::CROSSHAIR_GAP_LENGTH;
    let pattern_length = dash_length + gap_length;
    let crosshair_line_color = Color::srgba(1.0, 1.0, 1.0, config::CROSSHAIR_LINE_ALPHA);
    let crosshair_label_color = Color::srgb(1.0, 1.0, 0.0); // Yellow

    // Spawn vertical line segments (dashed pattern)
    let num_segments = (total_height / pattern_length).ceil() as usize;
    let mut vertical_line_segments = Vec::new();

    for i in 0..num_segments {
        let segment_y = chart_bottom + (i as f32 * pattern_length) + (dash_length / 2.0);
        let segment_id = commands
            .spawn((
                Sprite {
                    color: crosshair_line_color,
                    custom_size: Some(Vec2::new(config::CROSSHAIR_LINE_THICKNESS, dash_length)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0.0, segment_y, config::Z_LAYER_CROSSHAIR_LINES)),
                Visibility::Hidden,
                CrosshairElement,
            ))
            .id();
        vertical_line_segments.push(segment_id);
    }

    // Spawn horizontal lines and price labels (one per pane)
    for pane in &pane_manager.panes {
        let viewport = &pane.space.viewport;

        // Horizontal line segments for this pane (dashed pattern)
        let pane_width = viewport.width();
        let num_h_segments = (pane_width / pattern_length).ceil() as usize;
        let mut h_segments = Vec::new();

        for i in 0..num_h_segments {
            let segment_x = viewport.min.x + (i as f32 * pattern_length) + (dash_length / 2.0);
            let segment_id = commands
                .spawn((
                    Sprite {
                        color: crosshair_line_color,
                        custom_size: Some(Vec2::new(dash_length, config::CROSSHAIR_LINE_THICKNESS)),
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(segment_x, 0.0, config::Z_LAYER_CROSSHAIR_LINES)),
                    Visibility::Hidden,
                    CrosshairElement,
                ))
                .id();
            h_segments.push(segment_id);
        }
        horizontal_lines.push((pane.id, h_segments));

        // Price label for this pane
        let label = commands
            .spawn((
                Text2d::new(""),
                TextFont {
                    font_size: config::CROSSHAIR_LABEL_FONT_SIZE,
                    ..default()
                },
                TextColor(crosshair_label_color),
                Transform::from_translation(Vec3::new(chart_right + config::PRICE_LABEL_OFFSET_X, 0.0, config::Z_LAYER_CROSSHAIR_LABELS)),
                bevy::sprite::Anchor::CENTER_LEFT,
                Visibility::Hidden,
                CrosshairElement,
            ))
            .id();
        price_labels.push((pane.id, label));
    }

    // Spawn time label
    let time_label = commands
        .spawn((
            Text2d::new(""),
            TextFont {
                font_size: config::CROSSHAIR_LABEL_FONT_SIZE,
                ..default()
            },
            TextColor(crosshair_label_color),
            Transform::from_translation(Vec3::new(0.0, chart_bottom - config::TIME_LABEL_OFFSET_Y, config::Z_LAYER_CROSSHAIR_LABELS)),
            bevy::sprite::Anchor::CENTER,
            Visibility::Hidden,
            CrosshairElement,
        ))
        .id();

    // Spawn OHLCV info box
    let ohlcv_box = commands
        .spawn((
            Text2d::new(""),
            TextFont {
                font_size: config::OHLCV_BOX_FONT_SIZE,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 1.0)), // White
            Transform::from_translation(Vec3::new(
                first_pane.space.viewport.min.x + config::OHLCV_BOX_OFFSET_X,
                first_pane.space.viewport.max.y - config::OHLCV_BOX_OFFSET_Y,
                config::Z_LAYER_CROSSHAIR_LABELS,
            )),
            bevy::sprite::Anchor::TOP_LEFT,
            Visibility::Hidden,
            CrosshairElement,
        ))
        .id();

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

// ============================================================================
// GRID SYSTEMS (Split from render_grid_and_axes)
// ============================================================================

/// Render horizontal and vertical grid lines within each pane
pub fn render_grid_lines(
    mut commands: Commands,
    viewport_state: Res<ViewportState>,
    pane_manager: Res<PaneManager>,
    candle_data: Res<CandleData>,
    grid: Res<ChartGrid>,
    z_layers: Res<ZLayerConfig>,
    query: Query<Entity, With<GridLineElement>>,
) {
    use crate::config;

    if !viewport_state.needs_redraw {
        return;
    }

    // Despawn existing grid line elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if !grid.show_grid || pane_manager.panes.is_empty() {
        return;
    }

    // Calculate chart bounds
    let first_pane = &pane_manager.panes[0];
    let chart_left = first_pane.space.viewport.min.x;
    let chart_right = first_pane.space.viewport.max.x;

    // ========== HORIZONTAL GRID LINES (per pane) ==========
    for pane in &pane_manager.panes {
        let viewport = &pane.space.viewport;

        for i in 0..=grid.y_tick_count {
            let value_percent = i as f32 / grid.y_tick_count as f32;
            let y = viewport.min.y + value_percent * viewport.height();

            // Draw horizontal line
            let line_center = Vec2::new((chart_left + chart_right) / 2.0, y);
            let line_width = chart_right - chart_left;

            commands.spawn((
                Sprite {
                    color: grid.grid_color,
                    custom_size: Some(Vec2::new(line_width, config::GRID_LINE_THICKNESS)),
                    ..default()
                },
                Transform::from_translation(line_center.extend(z_layers.grid)),
                GridLineElement,
            ));
        }
    }

    // ========== VERTICAL GRID LINES (per pane) ==========
    for pane in &pane_manager.panes {
        let viewport = &pane.space.viewport;

        for i in 0..=grid.x_tick_count {
            let candle_percent = i as f32 / grid.x_tick_count as f32;
            let candle_index = viewport_state.visible_candle_start
                + (candle_percent * viewport_state.visible_candle_count as f32) as usize;

            if candle_index >= candle_data.candles.len() {
                continue;
            }

            let x = chart_left + candle_percent * (chart_right - chart_left);

            // Draw vertical line within this pane only
            let line_center = Vec2::new(x, viewport.center().y);
            let line_height = viewport.height();

            commands.spawn((
                Sprite {
                    color: grid.grid_color,
                    custom_size: Some(Vec2::new(config::GRID_LINE_THICKNESS, line_height)),
                    ..default()
                },
                Transform::from_translation(line_center.extend(z_layers.grid)),
                GridLineElement,
            ));
        }
    }
}

/// Render borders around each pane (panel-style separation)
pub fn render_pane_borders(
    mut commands: Commands,
    viewport_state: Res<ViewportState>,
    pane_manager: Res<PaneManager>,
    theme: Res<ChartTheme>,
    dimensions: Res<ChartDimensions>,
    z_layers: Res<ZLayerConfig>,
    query: Query<Entity, With<PaneBorderElement>>,
) {
    if !viewport_state.needs_redraw {
        return;
    }

    // Despawn existing border elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if pane_manager.panes.is_empty() {
        return;
    }

    let border_color = theme.pane_border;
    let border_thickness = dimensions.border_thickness;

    for pane in &pane_manager.panes {
        let viewport = &pane.space.viewport;

        // Top border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.center().x, viewport.max.y, z_layers.pane_borders)),
            PaneBorderElement,
        ));

        // Bottom border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.center().x, viewport.min.y, z_layers.pane_borders)),
            PaneBorderElement,
        ));

        // Left border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.min.x, viewport.center().y, z_layers.pane_borders)),
            PaneBorderElement,
        ));

        // Right border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.max.x, viewport.center().y, z_layers.pane_borders)),
            PaneBorderElement,
        ));
    }
}

/// Render resize grips in the gaps between panes
pub fn render_resize_grips(
    mut commands: Commands,
    viewport_state: Res<ViewportState>,
    pane_manager: Res<PaneManager>,
    interaction: Res<InteractionState>,
    theme: Res<ChartTheme>,
    dimensions: Res<ChartDimensions>,
    z_layers: Res<ZLayerConfig>,
    query: Query<Entity, With<ResizeGripElement>>,
) {
    if !viewport_state.needs_redraw {
        return;
    }

    // Despawn existing resize grip elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if pane_manager.panes.len() < 2 {
        return; // Need at least 2 panes to have resize grips
    }

    // Calculate chart bounds
    let first_pane = &pane_manager.panes[0];
    let chart_left = first_pane.space.viewport.min.x;
    let chart_right = first_pane.space.viewport.max.x;
    let gap_center_x = (chart_left + chart_right) / 2.0;

    let grip_width = dimensions.resize_grip_width;
    let line_height = dimensions.resize_grip_line_height;
    let line_spacing = dimensions.resize_grip_line_spacing;

    // Render resize grips between panes
    for i in 0..pane_manager.panes.len() - 1 {
        let pane_bottom = pane_manager.panes[i].space.viewport.min.y;
        let next_pane_top = pane_manager.panes[i + 1].space.viewport.max.y;
        let gap_center_y = (pane_bottom + next_pane_top) / 2.0;

        // Highlight grip if hovering or resizing this gap
        let is_active = interaction.hover_resize_gap == Some(i) || interaction.resizing_gap == Some(i);
        let grip_color = if is_active {
            theme.resize_grip_active
        } else {
            theme.resize_grip_inactive
        };

        // Draw 2 horizontal lines as grip indicator
        for j in 0..2 {
            let offset = (j as f32 - 0.5) * (line_height + line_spacing);
            let line_y = gap_center_y + offset;

            commands.spawn((
                Sprite {
                    color: grip_color,
                    custom_size: Some(Vec2::new(grip_width, line_height)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(gap_center_x, line_y, z_layers.resize_grips)),
                ResizeGripElement,
            ));
        }
    }
}

/// Render axis labels (X-axis time labels and Y-axis price labels)
pub fn render_axis_labels(
    mut commands: Commands,
    viewport_state: Res<ViewportState>,
    pane_manager: Res<PaneManager>,
    candle_data: Res<CandleData>,
    grid: Res<ChartGrid>,
    axes: Res<ChartAxes>,
    dimensions: Res<ChartDimensions>,
    z_layers: Res<ZLayerConfig>,
    query: Query<Entity, With<AxisLabelElement>>,
) {
    if !viewport_state.needs_redraw {
        return;
    }

    // Despawn existing axis label elements
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    if pane_manager.panes.is_empty() {
        return;
    }

    // Calculate chart bounds
    let first_pane = &pane_manager.panes[0];
    let last_pane = &pane_manager.panes[pane_manager.panes.len() - 1];
    let chart_bottom = last_pane.space.viewport.min.y;
    let chart_left = first_pane.space.viewport.min.x;
    let chart_right = first_pane.space.viewport.max.x;

    // ========== Y-AXIS LABELS (per pane, on the right side) ==========
    if axes.show_y_labels {
        for pane in &pane_manager.panes {
            let viewport = &pane.space.viewport;

            for i in 0..=grid.y_tick_count {
                let value_percent = i as f32 / grid.y_tick_count as f32;
                let value = pane.space.visible_price_min
                    + value_percent * (pane.space.visible_price_max - pane.space.visible_price_min);
                let y = viewport.min.y + value_percent * viewport.height();

                let label_x = chart_right + dimensions.price_label_offset_x;
                let label_text = format!("{:.2}", value);

                commands.spawn((
                    Text2d::new(label_text),
                    TextFont {
                        font_size: dimensions.axis_label_font_size,
                        ..default()
                    },
                    TextColor(axes.label_color),
                    Transform::from_translation(Vec3::new(label_x, y, z_layers.axis_labels)),
                    bevy::sprite::Anchor::CENTER_LEFT,
                    AxisLabelElement,
                ));
            }
        }
    }

    // ========== X-AXIS TIME LABELS (at bottom of chart) ==========
    if axes.show_x_labels {
        for i in 0..=grid.x_tick_count {
            let candle_percent = i as f32 / grid.x_tick_count as f32;
            let candle_index = viewport_state.visible_candle_start
                + (candle_percent * viewport_state.visible_candle_count as f32) as usize;

            if candle_index >= candle_data.candles.len() {
                continue;
            }

            let x = chart_left + candle_percent * (chart_right - chart_left);
            let candle = &candle_data.candles[candle_index];
            let label_y = chart_bottom - dimensions.time_label_offset_y;

            // Format timestamp using chrono
            let datetime =
                DateTime::<Utc>::from_timestamp(candle.time / 1000, 0).unwrap_or_default();
            let label_text = datetime.format("%m/%d %H:%M").to_string();

            commands.spawn((
                Text2d::new(label_text),
                TextFont {
                    font_size: dimensions.axis_label_font_size,
                    ..default()
                },
                TextColor(axes.label_color),
                Transform::from_translation(Vec3::new(x, label_y, z_layers.axis_labels)),
                bevy::sprite::Anchor::CENTER,
                AxisLabelElement,
            ));
        }
    }
}

pub fn update_crosshair(
    crosshair_entities: Res<CrosshairEntities>,
    candle_data: Res<CandleData>,
    viewport_state: Res<ViewportState>,
    pane_manager: Res<PaneManager>,
    crosshair: Res<Crosshair>,
    mut crosshair_state: ResMut<CrosshairState>,
    interaction: Res<InteractionState>,
    mut transforms: Query<&mut Transform>,
    mut visibilities: Query<&mut Visibility>,
    mut texts: Query<&mut Text2d>,
    mut cursor_options: Query<&mut CursorOptions>,
) {
    // Update crosshair state from interaction
    crosshair_state.mouse_pos = interaction.mouse_pos;

    if !crosshair.enabled || pane_manager.panes.is_empty() {
        // Hide all crosshair elements and show cursor
        crosshair_state.visible = false;
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
        for mut cursor in cursor_options.iter_mut() {
            cursor.visible = true;
        }
        return;
    }

    // Check if mouse is within any pane
    let mouse_in_chart = pane_manager
        .panes
        .iter()
        .any(|pane| pane.space.viewport.contains(interaction.mouse_pos));

    crosshair_state.visible = mouse_in_chart;

    // Show cursor if: outside chart, dragging, or resizing
    // Hide cursor only when: in chart AND not interacting
    let should_hide_cursor = mouse_in_chart
        && !interaction.dragging
        && interaction.hover_resize_gap.is_none()
        && interaction.resizing_gap.is_none();

    for mut cursor in cursor_options.iter_mut() {
        cursor.visible = !should_hide_cursor;
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
    for pane in &pane_manager.panes {
        let viewport = &pane.space.viewport;

        // Find entities for this pane
        let h_line_segments = crosshair_entities
            .horizontal_lines
            .iter()
            .find(|(id, _)| *id == pane.id)
            .map(|(_, segments)| segments);
        let label_entity = crosshair_entities
            .price_labels
            .iter()
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
                        viewport_state.visible_candle_start,
                        viewport_state.visible_candle_count,
                    );

                    if let Ok(mut text) = texts.get_mut(entity) {
                        **text = format!("{:.2}", value_at_cursor);
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

    // ========== FIND CANDLE AT CURSOR ==========
    let (candle_index, _) = if let Some(pane) = pane_manager.panes.first() {
        pane.space.from_world(
            interaction.mouse_pos,
            viewport_state.visible_candle_start,
            viewport_state.visible_candle_count,
        )
    } else {
        return;
    };

    if candle_index >= candle_data.candles.len() {
        // Hide time label and OHLCV box if no valid candle
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
            *vis = Visibility::Hidden;
        }
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
            *vis = Visibility::Hidden;
        }
        crosshair_state.hovered_candle = None;
        return;
    }

    // ========== DEBOUNCE: Only update text if candle changed ==========
    let candle_changed = crosshair_state.hovered_candle != Some(candle_index);

    if candle_changed {
        crosshair_state.hovered_candle = Some(candle_index);
        let candle = &candle_data.candles[candle_index];

        // ========== UPDATE TIME LABEL (only when candle changes) ==========
        if crosshair.show_time_label {
            let datetime =
                DateTime::<Utc>::from_timestamp(candle.time / 1000, 0).unwrap_or_default();
            let label_text = datetime.format("%m/%d %H:%M").to_string();

            if let Ok(mut text) = texts.get_mut(crosshair_entities.time_label) {
                **text = label_text;
            }
        }

        // ========== UPDATE OHLCV INFO BOX (only when candle changes) ==========
        if crosshair.show_ohlcv_box {
            let info_text = format!(
                "O: {:.2}  H: {:.2}  L: {:.2}  C: {:.2}\nVol: {:.2}",
                candle.open, candle.high, candle.low, candle.close, candle.volume
            );

            if let Ok(mut text) = texts.get_mut(crosshair_entities.ohlcv_box) {
                **text = info_text;
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
    } else if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
        *vis = Visibility::Hidden;
    }

    // OHLCV box visibility
    if crosshair.show_ohlcv_box {
        if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
            *vis = Visibility::Visible;
        }
    } else if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
        *vis = Visibility::Hidden;
    }
}

pub fn render_moving_averages(
    mut gizmos: Gizmos,
    viewport: Res<ViewportState>,
    pane_manager: Res<PaneManager>,
    candle_data: Res<CandleData>,
    indicator_state: Res<IndicatorState>,
) {
    // Find the Price pane (indicators overlay on price)
    let price_pane = match pane_manager.price_pane() {
        Some(pane) => pane,
        None => return,
    };

    // Get shared X-axis state
    let start = viewport.visible_candle_start;
    let end = (start + viewport.visible_candle_count).min(candle_data.candles.len());

    if start >= end {
        return;
    }

    // Render each Moving Average using Gizmos for smooth continuous lines
    for ma in &indicator_state.indicators {
        if !ma.visible || ma.values.is_empty() {
            continue;
        }

        // Draw continuous line connecting MA points
        for i in start..(end - 1) {
            // Need both current and next values to draw a line segment
            if let (Some(curr_value), Some(next_value)) =
                (ma.values[i], ma.values.get(i + 1).and_then(|v| *v))
            {
                // Convert to world coordinates
                let curr_pos = price_pane.space.to_world(
                    i,
                    curr_value,
                    viewport.visible_candle_start,
                    viewport.visible_candle_count,
                );

                let next_pos = price_pane.space.to_world(
                    i + 1,
                    next_value,
                    viewport.visible_candle_start,
                    viewport.visible_candle_count,
                );

                // Draw line segment with Gizmos (no gaps or artifacts!)
                gizmos.line_2d(curr_pos, next_pos, ma.color);
            }
        }
    }
}
