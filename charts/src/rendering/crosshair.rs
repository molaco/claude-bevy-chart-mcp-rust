use crate::cache::RenderCache;
use crate::types::*;
use crate::theme::ChartColors;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::CursorOptions;
use chrono::{DateTime, Utc};

use super::helpers::find_nearest_candle;

/// Initialize persistent crosshair entities (called once at startup from setup)
pub fn init_crosshair(commands: &mut Commands, chart: &Chart, colors: &ChartColors) {
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
        let segment_id = commands
            .spawn((
                Sprite {
                    color: colors.crosshair,
                    custom_size: Some(Vec2::new(1.0, DASH_LENGTH)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0.0, segment_y, 3.0)),
                Visibility::Hidden,
                CrosshairElement,
            ))
            .id();
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
            let segment_id = commands
                .spawn((
                    Sprite {
                        color: colors.crosshair,
                        custom_size: Some(Vec2::new(DASH_LENGTH, 1.0)),
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(segment_x, 0.0, 3.0)),
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
                    font_size: 14.0,
                    ..default()
                },
                TextColor(colors.text),
                Anchor::CENTER_LEFT,
                Transform::from_translation(Vec3::new(chart_right + 50.0, 0.0, 4.0)),
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
                font_size: 14.0,
                ..default()
            },
            TextColor(colors.text),
            Anchor::CENTER,
            Transform::from_translation(Vec3::new(0.0, chart_bottom - 40.0, 4.0)),
            Visibility::Hidden,
            CrosshairElement,
        ))
        .id();

    // Spawn OHLCV info box
    let ohlcv_box = commands
        .spawn((
            Text2d::new(""),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(colors.text),
            Anchor::TOP_LEFT,
            Transform::from_translation(Vec3::new(
                first_pane.space.viewport.min.x + 100.0,
                first_pane.space.viewport.max.y - 40.0,
                4.0,
            )),
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

/// Helper function to set visibility for GLOBAL crosshair elements
/// Note: Does NOT set visibility for per-pane elements (horizontal lines, price labels)
/// as those require selective visibility based on mouse Y position
fn set_crosshair_visibility(
    crosshair_entities: &CrosshairEntities,
    visibilities: &mut Query<&mut Visibility>,
    target_visibility: Visibility,
) {
    // Vertical line (global - spans all panes)
    for segment in &crosshair_entities.vertical_line_segments {
        if let Ok(mut vis) = visibilities.get_mut(*segment) {
            *vis = target_visibility;
        }
    }
    // Time label (global)
    if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.time_label) {
        *vis = target_visibility;
    }
    // OHLCV box (global)
    if let Ok(mut vis) = visibilities.get_mut(crosshair_entities.ohlcv_box) {
        *vis = target_visibility;
    }

    // Hide per-pane elements when crosshair is globally hidden
    if target_visibility == Visibility::Hidden {
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
    }
}

pub fn update_crosshair(
    crosshair_entities: Res<CrosshairEntities>,
    chart: Res<Chart>,
    crosshair: Res<Crosshair>,
    mut interaction: ResMut<InteractionState>,
    mut render_cache: ResMut<RenderCache>,
    mut transforms: Query<&mut Transform>,
    mut visibilities: Query<&mut Visibility>,
    mut texts: Query<&mut Text>,
    mut cursor_options: Query<&mut CursorOptions, With<Window>>,
) {
    // ========== OPTIMIZATION: Calculate visibility state ONCE ==========
    let mouse_in_chart = !chart.panes.is_empty()
        && chart
            .panes
            .iter()
            .any(|pane| pane.space.viewport.contains(interaction.mouse_pos));

    let should_show_crosshair = crosshair.enabled
        && !chart.panes.is_empty()
        && mouse_in_chart
        && !interaction.dragging
        && interaction.hover_resize_gap.is_none()
        && interaction.resizing_gap.is_none();

    // ========== OPTIMIZATION: Only update visibility on state change ==========
    if interaction.crosshair_visible != should_show_crosshair {
        interaction.crosshair_visible = should_show_crosshair;

        let target_visibility = if should_show_crosshair {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        set_crosshair_visibility(&crosshair_entities, &mut visibilities, target_visibility);

        #[cfg(debug_assertions)]
        println!("Crosshair visibility changed: {}", should_show_crosshair);
    }

    // Update cursor visibility
    let should_hide_cursor = should_show_crosshair;
    for mut opts in cursor_options.iter_mut() {
        opts.visible = !should_hide_cursor;
    }

    // Early return if crosshair should not be shown
    if !should_show_crosshair {
        return;
    }

    // ========== FROM HERE ON: Crosshair is visible, update positions ==========
    let time_start = chart.visible_time_start;
    let time_end = chart.visible_time_end;

    let first_pane = &chart.panes[0];
    let chart_right = first_pane.space.viewport.max.x;

    let mouse_x = interaction.mouse_pos.x;
    let mouse_y = interaction.mouse_pos.y;

    // ========== OPTIMIZATION: Only update positions if mouse moved ==========
    let mouse_moved = interaction.mouse_pos != interaction.last_crosshair_mouse_pos;

    if mouse_moved {
        interaction.last_crosshair_mouse_pos = interaction.mouse_pos;

        // Selective cache invalidation: only clear crosshair, preserve main chart cache
        render_cache.clear_crosshair();

        // NOTE: Vertical line X position is now updated below (snapped to candle center)
        // after find_nearest_candle() is called

        // ========== UPDATE PER-PANE HORIZONTAL LINES & LABELS ==========
        for pane in &chart.panes {
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
                // Update horizontal line segments position AND show them
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
                            time_start,
                            time_end,
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
                // Mouse NOT in this pane - hide horizontal lines and label
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

    // ========== FIND NEAREST CANDLE AT CURSOR (Phase 4: Time-based) ==========
    // Convert mouse X to timestamp
    let (cursor_time, _) = if let Some(pane) = chart.panes.first() {
        pane.space.from_world(
            interaction.mouse_pos,
            time_start,
            time_end,
        )
    } else {
        return;
    };

    // Find nearest candle by timestamp (O(log n) BTreeMap lookup)
    let candle = find_nearest_candle(&chart.candles, cursor_time);
    if candle.is_none() {
        interaction.last_crosshair_candle_timestamp = None;
        return;
    }
    let (candle_time, candle) = candle.unwrap();

    // ========== SNAP CROSSHAIR X TO CANDLE CENTER ==========
    let snapped_x = chart.panes[0]
        .space
        .to_world(*candle_time, candle.close as f32, time_start, time_end)
        .x;

    // Update vertical line position (snapped to candle center)
    for segment in &crosshair_entities.vertical_line_segments {
        if let Ok(mut transform) = transforms.get_mut(*segment) {
            transform.translation.x = snapped_x;
        }
    }

    // ========== DEBOUNCE: Only update text if candle changed ==========
    let candle_changed = interaction.last_crosshair_candle_timestamp != Some(*candle_time);

    if candle_changed {
        interaction.last_crosshair_candle_timestamp = Some(*candle_time);

        // ========== UPDATE TIME LABEL TEXT (only when candle changes) ==========
        if crosshair.show_time_label {
            let datetime =
                DateTime::<Utc>::from_timestamp(*candle_time / 1000, 0).unwrap_or_default();
            let label_text = datetime.format("%Y-%m-%d %H:%M").to_string();

            if let Ok(mut text) = texts.get_mut(crosshair_entities.time_label) {
                text.0 = label_text;
            }
        }

        // ========== UPDATE OHLCV INFO BOX TEXT (only when candle changes) ==========
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

    // ========== UPDATE TIME LABEL POSITION (snapped to candle center) ==========
    if crosshair.show_time_label {
        if let Ok(mut transform) = transforms.get_mut(crosshair_entities.time_label) {
            transform.translation.x = snapped_x;
        }
        // Visibility already set by state change handler above
    }
}
