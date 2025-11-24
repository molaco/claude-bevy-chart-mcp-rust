use crate::cache::RenderCache;
use crate::types::*;
use crate::theme::ChartColors;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use chrono::{DateTime, Utc};

use super::helpers::format_volume;

pub fn render_grid_and_axes(
    mut commands: Commands,
    chart: Res<Chart>,
    grid: Res<ChartGrid>,
    axes: Res<ChartAxes>,
    colors: Res<ChartColors>,
    config: Res<CandlestickLODConfig>,
    mut cache: ResMut<RenderCache>,
    grid_lines_query: Query<Entity, With<GridLine>>,
    x_labels_query: Query<Entity, With<XAxisLabel>>,
    y_labels_query: Query<Entity, With<YAxisLabel>>,
) {
    if !chart.needs_redraw {
        return;
    }

    let x_labels_dirty = cache.is_x_labels_dirty();
    let y_labels_dirty = cache.is_y_labels_dirty();

    // Always despawn grid lines (they're cheap sprites)
    for entity in grid_lines_query.iter() {
        commands.entity(entity).despawn();
    }

    // Only despawn X labels if dirty (text is expensive)
    if x_labels_dirty {
        for entity in x_labels_query.iter() {
            commands.entity(entity).despawn();
        }
    }

    // Only despawn Y labels if dirty (text is expensive)
    if y_labels_dirty {
        for entity in y_labels_query.iter() {
            commands.entity(entity).despawn();
        }
    }

    if !grid.show_grid || chart.panes.is_empty() {
        return;
    }

    let time_start = chart.visible_time_start;
    let time_end = chart.visible_time_end;

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

        // For volume panes, calculate render height ratio (based on configured padding)
        let render_height_ratio = match pane.pane_type {
            PaneType::Volume => {
                // visible_price_max = actual_max * volume_y_axis_padding
                // Grid lines should only go up to actual_max / visible_price_max = 1/padding
                1.0 / config.volume_y_axis_padding
            }
            _ => 1.0,
        };

        for i in 0..=grid.y_tick_count {
            let value_percent = i as f32 / grid.y_tick_count as f32;

            // For volume panes, don't render grid lines above the actual data range
            if value_percent > render_height_ratio {
                continue;
            }

            let value = pane.space.visible_price_min
                + value_percent * (pane.space.visible_price_max - pane.space.visible_price_min);

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
                GridLine,
            ));

            // Y-axis label on the right side (only if dirty)
            if axes.show_y_labels && y_labels_dirty {
                let label_x = chart_right + 50.0;

                // Format based on pane type
                let label_text = match pane.pane_type {
                    PaneType::Volume => format_volume(value / config.volume_y_axis_padding),
                    _ => format!("{:.2}", value),
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
                    YAxisLabel,
                ));
            }
        }
    }

    // ========== PANE BORDERS (Panel-style separation) ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;
        let border_color = colors.axis_line;
        let border_thickness = 2.0;

        // Top border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.center().x, viewport.max.y, 0.4)),
            GridLine,
        ));

        // Bottom border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.center().x, viewport.min.y, 0.4)),
            GridLine,
        ));

        // Left border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.min.x, viewport.center().y, 0.4)),
            GridLine,
        ));

        // Right border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.max.x, viewport.center().y, 0.4)),
            GridLine,
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
        let grip_color = colors.axis_line;

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
                GridLine,
            ));
        }
    }

    // ========== VERTICAL GRID LINES (Time - per pane) ==========
    for pane in &chart.panes {
        let viewport = &pane.space.viewport;

        for i in 0..=grid.x_tick_count {
            let t = i as f32 / grid.x_tick_count as f32;
            let x = chart_left + t * (chart_right - chart_left);

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
                GridLine,
            ));
        }
    }

    // ========== X-AXIS TIME LABELS (at bottom of last pane) ==========
    if axes.show_x_labels && x_labels_dirty {
        let time_range = time_end - time_start;
        for i in 0..=grid.x_tick_count {
            let t = i as f32 / grid.x_tick_count as f32;
            let timestamp = time_start + (t * time_range as f32) as i64;

            let x = chart_left + t * (chart_right - chart_left);
            let label_y = chart_bottom - 40.0;

            // Format timestamp using chrono
            let datetime =
                DateTime::<Utc>::from_timestamp(timestamp / 1000, 0).unwrap_or_default();
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
                XAxisLabel,
            ));
        }
    }

    // Mark label caches as clean after rendering
    if x_labels_dirty {
        cache.mark_x_labels_clean();
    }
    if y_labels_dirty {
        cache.mark_y_labels_clean();
    }
}
