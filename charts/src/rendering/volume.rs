use crate::types::*;
use crate::aggregation::*;
use crate::theme::ChartColors;
use bevy::prelude::*;

pub fn render_volume_bars(
    mut commands: Commands,
    chart: Res<Chart>,
    config: Res<CandlestickLODConfig>,
    colors: Res<ChartColors>,
    agg_config: Res<AggregationConfig>,
    mut agg_cache: ResMut<AggregationCache>,
    agg_state: Res<AggregationState>,  // Read-only: level detection done in detect_aggregation_level_change
    mut pools: ResMut<EntityPools>,
    mut query: Query<(
        Entity,
        &mut VolumeBar,
        &mut Transform,
        &mut Sprite,
        &mut Visibility,
    )>,
    mut pooled_query: Query<&mut PooledEntity>,
) {
    if !chart.needs_redraw || chart.load_status != ChartLoadStatus::Ready {
        return;
    }

    let volume_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Volume));
    let price_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price));
    if volume_pane.is_none() || price_pane.is_none() {
        return;
    }
    let volume_pane = volume_pane.unwrap();
    let price_pane = price_pane.unwrap();

    let time_start = chart.visible_time_start;
    let time_end = chart.visible_time_end;

    // Calculate max volume for Y-scaling
    let max_volume = chart.candles
        .range(time_start..=time_end)
        .map(|(_, c)| c.volume as f32)
        .fold(0.0f32, f32::max) * config.volume_y_axis_padding;

    let visible_count = chart.candles.range(time_start..=time_end).count();
    let candle_width_px = price_pane.space.viewport.width() / visible_count.max(1) as f32;

    if candle_width_px < config.volume_render_threshold {
        // Hide all volume bars
        for (entity, _, _, _, mut visibility) in query.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.volume_bars.return_entity(entity, PooledEntityType::VolumeBar);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }
        return;
    }

    // Use pre-computed aggregation level from detect_aggregation_level_change system
    // This ensures all rendering systems see the same level (no race condition)
    let level = agg_state.current_level;

    // Track if we cleared pools this frame
    let pools_just_cleared = agg_state.level_changed_this_frame;
    if pools_just_cleared {
        // Return all volume bars to pool
        for (entity, _, _, _, mut visibility) in query.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.volume_bars.return_entity(entity, PooledEntityType::VolumeBar);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }
    }

    // Collect existing entities by candle_timestamp
    use std::collections::{HashMap, HashSet};
    let mut existing_bars: HashMap<i64, Entity> = HashMap::new();

    if !pools_just_cleared {
        for (entity, bar, _, _, _) in query.iter() {
            existing_bars.insert(bar.candle_timestamp, entity);
        }
    }

    let mut updated_bars = HashSet::<Entity>::new();
    let mut spawned_count = 0;

    // DIRECT ITERATION
    for (timestamp, candle) in chart.candles.range(time_start..=time_end) {
        let ts = *timestamp;

        let bar_bottom = volume_pane.space.to_world_with_y_range(
            ts, 0.0, time_start, time_end, 0.0, max_volume
        );
        let bar_top = volume_pane.space.to_world_with_y_range(
            ts, candle.volume as f32, time_start, time_end, 0.0, max_volume
        );

        let bar_center = Vec2::new(
            (bar_bottom.x + bar_top.x) / 2.0,
            (bar_bottom.y + bar_top.y) / 2.0,
        );
        let bar_height = (bar_top.y - bar_bottom.y).abs().max(1.0);

        // Determine bar width based on candle size (LOD)
        let bar_width = if candle_width_px >= 2.0 {
            // Full detail: 70% of candle width
            candle_width_px * 0.7
        } else {
            // Thin detail: 30% of candle width when zoomed out
            candle_width_px * 0.3
        };

        // Color based on candle direction
        let bar_color = if candle.close >= candle.open {
            colors.volume_bull
        } else {
            colors.volume_bear
        };

        // UPDATE or GET FROM POOL volume bar entity
        if let Some(&entity) = existing_bars.get(&ts) {
            // Update existing bar
            if let Ok((_, _, mut transform, mut sprite, mut visibility)) = query.get_mut(entity) {
                transform.translation = bar_center.extend(0.0);
                // Only update color if it changed to prevent unnecessary change detection
                if sprite.color != bar_color {
                    sprite.color = bar_color;
                }
                if let Some(ref mut size) = sprite.custom_size {
                    *size = Vec2::new(bar_width, bar_height);
                }
                *visibility = Visibility::Visible;
            }
            updated_bars.insert(entity);
        } else if let Some(entity) = pools.volume_bars.get() {
            // Reuse from pool
            if let Ok((_, mut bar, mut transform, mut sprite, mut visibility)) =
                query.get_mut(entity)
            {
                bar.candle_timestamp = ts;
                transform.translation = bar_center.extend(0.0);
                sprite.color = bar_color;
                if let Some(ref mut size) = sprite.custom_size {
                    *size = Vec2::new(bar_width, bar_height);
                }
                *visibility = Visibility::Visible;
                if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                    pooled.in_use = true;
                }
                updated_bars.insert(entity);
            } else {
                // Entity from pool is invalid - remove it permanently and spawn new as fallback
                pools.volume_bars.remove_invalid(entity);

                // Spawn new entity to replace the corrupted one
                let new_entity = commands
                    .spawn((
                        Sprite {
                            color: bar_color,
                            custom_size: Some(Vec2::new(bar_width, bar_height)),
                            ..default()
                        },
                        Transform::from_translation(bar_center.extend(0.0)),
                        VolumeBar { candle_timestamp: ts },
                        PooledEntity {
                            entity_type: PooledEntityType::VolumeBar,
                            in_use: true,
                        },
                        PendingPoolEntry {
                            entity_type: PooledEntityType::VolumeBar,
                        },
                        VolumeElement,
                        PaneId::Volume,
                    ))
                    .id();
                // pools.volume_bars.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                updated_bars.insert(new_entity);
                spawned_count += 1;
            }
        } else {
            // Pool exhausted - spawn new
            // NOTE: Don't add to pool yet - entity doesn't exist until next frame!
            let new_entity = commands
                .spawn((
                    Sprite {
                        color: bar_color,
                        custom_size: Some(Vec2::new(bar_width, bar_height)),
                        ..default()
                    },
                    Transform::from_translation(bar_center.extend(0.0)),
                    VolumeBar { candle_timestamp: ts },
                    PooledEntity {
                        entity_type: PooledEntityType::VolumeBar,
                        in_use: true,
                    },
                    PendingPoolEntry {
                        entity_type: PooledEntityType::VolumeBar,
                    },
                    VolumeElement,
                    PaneId::Volume,
                ))
                .id();
            // pools.volume_bars.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
            updated_bars.insert(new_entity);
            spawned_count += 1;
        }
    }

    // Hide and return to pool entities that are no longer visible
    // Iterate through all entities (not HashMap) to avoid stale index issues
    let mut hidden_count = 0;
    for (entity, _, _, _, mut visibility) in query.iter_mut() {
        if !updated_bars.contains(&entity) {
            *visibility = Visibility::Hidden;
            pools.volume_bars.return_entity(entity, PooledEntityType::VolumeBar);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
            hidden_count += 1;
        }
    }

    #[cfg(debug_assertions)]
    {
        let (vol_total, vol_avail) = pools.volume_bars.stats();
        let mut log_msg = format!(
            "VOL: Rendered {} bars | Updated: {} | Spawned: {} | Hidden: {} | Pool: {}/{}",
            visible_count,
            updated_bars.len(),
            spawned_count,
            hidden_count,
            vol_total - vol_avail,
            vol_total
        );

        if pools_just_cleared {
            log_msg.push_str(" | POOLS CLEARED");
        }

        println!("{}", log_msg);
    }
}
