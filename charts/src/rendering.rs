use crate::types::*;
use crate::aggregation::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::CursorOptions;
use chrono::{DateTime, Utc};

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
    config: Res<CandlestickLODConfig>,
    agg_config: Res<AggregationConfig>,
    mut agg_cache: ResMut<AggregationCache>,
    mut agg_state: ResMut<AggregationState>,
    mut pools: ResMut<EntityPools>,
    mut query_wicks: Query<
        (
            Entity,
            &mut CandlestickWick,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
        ),
        (
            Without<CandlestickBody>,
            Without<CandlestickOHLCLine>,
            Without<CandlestickRangeLine>,
        ),
    >,
    mut query_bodies: Query<
        (
            Entity,
            &mut CandlestickBody,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
        ),
        (
            Without<CandlestickWick>,
            Without<CandlestickOHLCLine>,
            Without<CandlestickRangeLine>,
        ),
    >,
    mut query_ohlc: Query<
        (
            Entity,
            &mut CandlestickOHLCLine,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
        ),
        (
            Without<CandlestickWick>,
            Without<CandlestickBody>,
            Without<CandlestickRangeLine>,
        ),
    >,
    mut query_range: Query<
        (
            Entity,
            &mut CandlestickRangeLine,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
        ),
        (
            Without<CandlestickWick>,
            Without<CandlestickBody>,
            Without<CandlestickOHLCLine>,
        ),
    >,
    mut pooled_query: Query<&mut PooledEntity>,
) {
    if !chart.needs_redraw {
        return;
    }

    // Find the Price pane
    let price_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price));

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

    // Determine if aggregation is needed
    let level = if agg_config.enabled {
        agg_state.get_stable_level(
            chart.visible_candle_count,
            agg_config.max_renderable_candles,
        )
    } else {
        AggregationLevel::None
    };

    // Get aggregated data if needed (must live for entire function)
    let aggregated_data;
    let (candles_to_render, index_mapping_offset) = if level != AggregationLevel::None {
        aggregated_data = agg_cache.get_or_aggregate(
            &chart.timeframe,
            &chart.candles,
            start,
            chart.visible_candle_count,
            level,
        );

        #[cfg(debug_assertions)]
        println!(
            "Aggregation: {:?} ({}:1) - {} -> {} candles",
            level,
            level.ratio(),
            chart.visible_candle_count,
            aggregated_data.candles.len()
        );

        // Return aggregated candles with offset 0
        (aggregated_data.candles.as_slice(), 0_usize)
    } else {
        // No aggregation needed
        (&chart.candles[start..end], start)
    };

    // Determine LOD level based on candle width
    // Use aggregated candle count for width calculation if aggregation is active
    let effective_candle_count = if level != AggregationLevel::None {
        candles_to_render.len().max(1)
    } else {
        chart.visible_candle_count.max(1)
    };
    let candle_width_px = price_pane.space.viewport.width() / effective_candle_count as f32;
    let lod_level = calculate_lod_level(candle_width_px, &config);

    // Check if aggregation level changed since last frame
    // If it changed, return ALL entities to pool to force clean respawn
    if level != agg_state.previous_level {
        #[cfg(debug_assertions)]
        println!(
            "Aggregation level changed: {:?} -> {:?}, clearing entity pools",
            agg_state.previous_level, level
        );

        // Return all wicks to pool
        for (entity, _, _, _, mut visibility) in query_wicks.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.wicks.return_entity(entity, PooledEntityType::CandlestickWick);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }

        // Return all bodies to pool
        for (entity, _, _, _, mut visibility) in query_bodies.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.bodies.return_entity(entity, PooledEntityType::CandlestickBody);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }

        // Return all OHLC lines to pool
        for (entity, _, _, _, mut visibility) in query_ohlc.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.ohlc_lines.return_entity(entity, PooledEntityType::CandlestickOHLC);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }

        // Return all range lines to pool
        for (entity, _, _, _, mut visibility) in query_range.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.range_lines.return_entity(entity, PooledEntityType::CandlestickRange);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }
    }

    // Collect existing entities by candle_index
    use std::collections::HashMap;
    let mut existing_wicks: HashMap<usize, Entity> = HashMap::new();
    let mut existing_bodies: HashMap<usize, Entity> = HashMap::new();
    let mut existing_ohlc: HashMap<usize, Entity> = HashMap::new();
    let mut existing_range: HashMap<usize, Entity> = HashMap::new();

    // Only collect existing entities if aggregation level hasn't changed
    // If level changed, all entities were just cleared and have stale indices
    if level == agg_state.previous_level {
        for (entity, wick, _, _, _) in query_wicks.iter() {
            existing_wicks.insert(wick.candle_index, entity);
        }
        for (entity, body, _, _, _) in query_bodies.iter() {
            existing_bodies.insert(body.candle_index, entity);
        }
        for (entity, ohlc, _, _, _) in query_ohlc.iter() {
            existing_ohlc.insert(ohlc.candle_index, entity);
        }
        for (entity, range_line, _, _, _) in query_range.iter() {
            existing_range.insert(range_line.candle_index, entity);
        }
    }

    // Track which entities we updated (to avoid despawning them)
    let mut updated_wicks = std::collections::HashSet::new();
    let mut updated_bodies = std::collections::HashSet::new();
    let mut updated_ohlc = std::collections::HashSet::new();
    let mut updated_range = std::collections::HashSet::new();

    // Counters for debug output
    let mut spawned_count = 0;

    // Process each visible candle based on LOD level
    for (idx, candle) in candles_to_render.iter().enumerate() {
        // Map aggregated index to original world space
        let i = if level != AggregationLevel::None {
            // Each aggregated candle represents a group of `ratio` source candles.
            // The aggregated candle's timestamp is from the first candle in the group,
            // but we want to position it at the CENTER of the group for visual accuracy.
            // For example, with ratio=10:
            //   - idx=0 represents candles [0..10), centered at 4.5
            //   - idx=1 represents candles [10..20), centered at 14.5
            // Using the center_offset() method, we position at the floor of the center:
            //   - idx=0 → start + (0*10 + center_offset(10)) = start + 5
            //   - idx=1 → start + (1*10 + center_offset(10)) = start + 15
            start + (idx * level.ratio()) + level.center_offset()
        } else {
            index_mapping_offset + idx
        };

        match lod_level {
            CandleLODLevel::Full => {
                // Full detail: render wick + body (hide LOD entities if they exist)
                if let Some(&entity) = existing_ohlc.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_ohlc.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.ohlc_lines.return_entity(entity, PooledEntityType::CandlestickOHLC);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_range.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_range.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.range_lines.return_entity(entity, PooledEntityType::CandlestickRange);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }

                // Calculate positions using ChartSpace::to_world()
                let wick_bottom = price_pane.space.to_world(
                    i,
                    candle.low as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );
                let wick_top = price_pane.space.to_world(
                    i,
                    candle.high as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );
                let body_open = price_pane.space.to_world(
                    i,
                    candle.open as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );
                let body_close = price_pane.space.to_world(
                    i,
                    candle.close as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );

                let wick_center = Vec2::new(
                    (wick_bottom.x + wick_top.x) / 2.0,
                    (wick_bottom.y + wick_top.y) / 2.0,
                );
                let wick_height = (wick_top.y - wick_bottom.y).abs().max(1.0);

                // UPDATE or GET FROM POOL wick entity
                if let Some(&entity) = existing_wicks.get(&i) {
                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_wicks.get_mut(entity)
                    {
                        transform.translation = wick_center.extend(0.0);
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.0, wick_height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_wicks.insert(i);
                } else if let Some(entity) = pools.wicks.get() {
                    // Reuse from pool
                    if let Ok((_, mut wick, mut transform, mut sprite, mut visibility)) =
                        query_wicks.get_mut(entity)
                    {
                        wick.candle_index = i;
                        transform.translation = wick_center.extend(0.0);
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.0, wick_height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_wicks.insert(i);
                    } else {
                        // Return entity to pool if query failed
                        pools.wicks.return_entity(entity, PooledEntityType::CandlestickWick);
                    }
                } else {
                    // Pool exhausted - spawn new
                    let new_entity = commands
                        .spawn((
                            Sprite {
                                color: Color::srgb(0.5, 0.5, 0.5),
                                custom_size: Some(Vec2::new(1.0, wick_height)),
                                ..default()
                            },
                            Transform::from_translation(wick_center.extend(0.0)),
                            CandlestickWick { candle_index: i },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickWick,
                                in_use: true,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    pools.wicks.add_entity(new_entity);
                    spawned_count += 1;
                }

                // Calculate body properties
                let body_width = candle_width_px * 0.7;
                let body_height = (body_close.y - body_open.y).abs().max(1.0);
                let body_center = Vec2::new(
                    (body_open.x + body_close.x) / 2.0,
                    (body_open.y + body_close.y) / 2.0,
                );

                let body_color = if candle.close >= candle.open {
                    Color::srgb(0.0, 0.8, 0.2) // Green
                } else {
                    Color::srgb(0.9, 0.2, 0.2) // Red
                };

                // UPDATE or GET FROM POOL body entity
                if let Some(&entity) = existing_bodies.get(&i) {
                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_bodies.get_mut(entity)
                    {
                        transform.translation = body_center.extend(1.0);
                        sprite.color = body_color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(body_width, body_height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_bodies.insert(i);
                } else if let Some(entity) = pools.bodies.get() {
                    // Reuse from pool
                    if let Ok((_, mut body, mut transform, mut sprite, mut visibility)) =
                        query_bodies.get_mut(entity)
                    {
                        body.candle_index = i;
                        transform.translation = body_center.extend(1.0);
                        sprite.color = body_color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(body_width, body_height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_bodies.insert(i);
                    } else {
                        // Return entity to pool if query failed
                        pools.bodies.return_entity(entity, PooledEntityType::CandlestickBody);
                    }
                } else {
                    // Pool exhausted - spawn new
                    let new_entity = commands
                        .spawn((
                            Sprite {
                                color: body_color,
                                custom_size: Some(Vec2::new(body_width, body_height)),
                                ..default()
                            },
                            Transform::from_translation(body_center.extend(1.0)),
                            CandlestickBody { candle_index: i },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickBody,
                                in_use: true,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    pools.bodies.add_entity(new_entity);
                    spawned_count += 1;
                }
            }

            CandleLODLevel::Medium => {
                // Medium detail: single OHLC line (hide full detail entities if they exist)
                if let Some(&entity) = existing_wicks.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_wicks.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.wicks.return_entity(entity, PooledEntityType::CandlestickWick);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_bodies.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_bodies.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.bodies.return_entity(entity, PooledEntityType::CandlestickBody);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_range.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_range.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.range_lines.return_entity(entity, PooledEntityType::CandlestickRange);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }

                let low_pos = price_pane.space.to_world(
                    i,
                    candle.low as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );
                let high_pos = price_pane.space.to_world(
                    i,
                    candle.high as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );

                let center = Vec2::new(
                    (low_pos.x + high_pos.x) / 2.0,
                    (low_pos.y + high_pos.y) / 2.0,
                );
                let height = (high_pos.y - low_pos.y).abs().max(1.0);

                let color = if candle.close >= candle.open {
                    Color::srgb(0.0, 0.8, 0.2) // Green
                } else {
                    Color::srgb(0.9, 0.2, 0.2) // Red
                };

                // UPDATE or GET FROM POOL OHLC line entity
                if let Some(&entity) = existing_ohlc.get(&i) {
                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_ohlc.get_mut(entity)
                    {
                        transform.translation = center.extend(0.0);
                        sprite.color = color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.5, height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_ohlc.insert(i);
                } else if let Some(entity) = pools.ohlc_lines.get() {
                    // Reuse from pool
                    if let Ok((_, mut ohlc, mut transform, mut sprite, mut visibility)) =
                        query_ohlc.get_mut(entity)
                    {
                        ohlc.candle_index = i;
                        transform.translation = center.extend(0.0);
                        sprite.color = color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.5, height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_ohlc.insert(i);
                    } else {
                        // Return entity to pool if query failed
                        pools.ohlc_lines.return_entity(entity, PooledEntityType::CandlestickOHLC);
                    }
                } else {
                    // Pool exhausted - spawn new
                    let new_entity = commands
                        .spawn((
                            Sprite {
                                color,
                                custom_size: Some(Vec2::new(1.5, height)),
                                ..default()
                            },
                            Transform::from_translation(center.extend(0.0)),
                            CandlestickOHLCLine { candle_index: i },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickOHLC,
                                in_use: true,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    pools.ohlc_lines.add_entity(new_entity);
                    spawned_count += 1;
                }
            }

            CandleLODLevel::Low => {
                // Low detail: single range line (hide other entities if they exist)
                if let Some(&entity) = existing_wicks.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_wicks.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.wicks.return_entity(entity, PooledEntityType::CandlestickWick);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_bodies.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_bodies.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.bodies.return_entity(entity, PooledEntityType::CandlestickBody);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_ohlc.get(&i) {
                    if let Ok((_, _, _, _, mut visibility)) = query_ohlc.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.ohlc_lines.return_entity(entity, PooledEntityType::CandlestickOHLC);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }

                let low_pos = price_pane.space.to_world(
                    i,
                    candle.low as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );
                let high_pos = price_pane.space.to_world(
                    i,
                    candle.high as f32,
                    chart.visible_candle_start,
                    chart.visible_candle_count,
                );

                let center = Vec2::new(
                    (low_pos.x + high_pos.x) / 2.0,
                    (low_pos.y + high_pos.y) / 2.0,
                );
                let height = (high_pos.y - low_pos.y).abs().max(1.0);

                let color = if candle.close >= candle.open {
                    Color::srgb(0.0, 0.8, 0.2) // Green
                } else {
                    Color::srgb(0.9, 0.2, 0.2) // Red
                };

                // UPDATE or GET FROM POOL range line entity
                if let Some(&entity) = existing_range.get(&i) {
                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_range.get_mut(entity)
                    {
                        transform.translation = center.extend(0.0);
                        sprite.color = color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(0.5, height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_range.insert(i);
                } else if let Some(entity) = pools.range_lines.get() {
                    // Reuse from pool
                    if let Ok((_, mut range, mut transform, mut sprite, mut visibility)) =
                        query_range.get_mut(entity)
                    {
                        range.candle_index = i;
                        transform.translation = center.extend(0.0);
                        sprite.color = color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(0.5, height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_range.insert(i);
                    } else {
                        // Return entity to pool if query failed
                        pools.range_lines.return_entity(entity, PooledEntityType::CandlestickRange);
                    }
                } else {
                    // Pool exhausted - spawn new
                    let new_entity = commands
                        .spawn((
                            Sprite {
                                color,
                                custom_size: Some(Vec2::new(0.5, height)),
                                ..default()
                            },
                            Transform::from_translation(center.extend(0.0)),
                            CandlestickRangeLine { candle_index: i },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickRange,
                                in_use: true,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    pools.range_lines.add_entity(new_entity);
                    spawned_count += 1;
                }
            }
        }
    }

    // Hide and return to pool entities that are no longer visible
    let mut hidden_count = 0;
    for (index, entity) in existing_wicks.iter() {
        if !updated_wicks.contains(index) {
            if let Ok((_, _, _, _, mut visibility)) = query_wicks.get_mut(*entity) {
                *visibility = Visibility::Hidden;
                pools.wicks.return_entity(*entity, PooledEntityType::CandlestickWick);
                if let Ok(mut pooled) = pooled_query.get_mut(*entity) {
                    pooled.in_use = false;
                }
                hidden_count += 1;
            }
        }
    }
    for (index, entity) in existing_bodies.iter() {
        if !updated_bodies.contains(index) {
            if let Ok((_, _, _, _, mut visibility)) = query_bodies.get_mut(*entity) {
                *visibility = Visibility::Hidden;
                pools.bodies.return_entity(*entity, PooledEntityType::CandlestickBody);
                if let Ok(mut pooled) = pooled_query.get_mut(*entity) {
                    pooled.in_use = false;
                }
                hidden_count += 1;
            }
        }
    }
    for (index, entity) in existing_ohlc.iter() {
        if !updated_ohlc.contains(index) {
            if let Ok((_, _, _, _, mut visibility)) = query_ohlc.get_mut(*entity) {
                *visibility = Visibility::Hidden;
                pools.ohlc_lines.return_entity(*entity, PooledEntityType::CandlestickOHLC);
                if let Ok(mut pooled) = pooled_query.get_mut(*entity) {
                    pooled.in_use = false;
                }
                hidden_count += 1;
            }
        }
    }
    for (index, entity) in existing_range.iter() {
        if !updated_range.contains(index) {
            if let Ok((_, _, _, _, mut visibility)) = query_range.get_mut(*entity) {
                *visibility = Visibility::Hidden;
                pools.range_lines.return_entity(*entity, PooledEntityType::CandlestickRange);
                if let Ok(mut pooled) = pooled_query.get_mut(*entity) {
                    pooled.in_use = false;
                }
                hidden_count += 1;
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        let level = agg_state.current_level;
        let stats = agg_cache.stats();
        let (wick_total, wick_avail) = pools.wicks.stats();
        let (body_total, body_avail) = pools.bodies.stats();

        println!(
            "AGG: {:?} ({}:1) | Rendered: {}/{} | Cache: {:.1}% hit ({}/{}) | Pools: W:{}/{} B:{}/{}",
            level,
            level.ratio(),
            candles_to_render.len(),
            chart.visible_candle_count,
            stats.hit_rate() * 100.0,
            stats.hits,
            stats.hits + stats.misses,
            wick_total - wick_avail,
            wick_total,
            body_total - body_avail,
            body_total,
        );
    }
}

pub fn render_volume_bars(
    mut commands: Commands,
    chart: Res<Chart>,
    config: Res<CandlestickLODConfig>,
    agg_config: Res<AggregationConfig>,
    mut agg_cache: ResMut<AggregationCache>,
    mut agg_state: ResMut<AggregationState>,
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
    if !chart.needs_redraw {
        return;
    }

    // Find the Volume pane
    let volume_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Volume));

    if volume_pane.is_none() {
        return;
    }
    let volume_pane = volume_pane.unwrap();

    // Find the Price pane to get candle width
    let price_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price));

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

    // Determine if aggregation is needed
    let level = if agg_config.enabled {
        agg_state.get_stable_level(
            chart.visible_candle_count,
            agg_config.max_renderable_candles,
        )
    } else {
        AggregationLevel::None
    };

    // If aggregation level changed, return ALL volume bars to pool
    if level != agg_state.previous_level {
        println!("Aggregation level changed for volume bars, clearing pool");

        // Return all volume bars to pool
        for (entity, _, _, _, mut visibility) in query.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.volume_bars.return_entity(entity, PooledEntityType::VolumeBar);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }
    }

    // Get aggregated data if needed (must live for entire function)
    let aggregated_data;
    let (candles_to_render, index_mapping_offset) = if level != AggregationLevel::None {
        aggregated_data = agg_cache.get_or_aggregate(
            &chart.timeframe,
            &chart.candles,
            start,
            chart.visible_candle_count,
            level,
        );

        // Return aggregated candles with offset 0
        (aggregated_data.candles.as_slice(), 0_usize)
    } else {
        // No aggregation needed
        (&chart.candles[start..end], start)
    };

    let volume_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Volume)).unwrap();

    // Check if candles are too small to render volume bars
    // Use aggregated candle count for width calculation if aggregation is active
    let effective_candle_count = if level != AggregationLevel::None {
        candles_to_render.len().max(1)
    } else {
        chart.visible_candle_count.max(1)
    };
    let candle_width_px = price_pane.space.viewport.width() / effective_candle_count as f32;

    if candle_width_px < config.volume_render_threshold {
        // Hide all volume bars and return to pool
        for (entity, _, _, _, mut visibility) in query.iter_mut() {
            *visibility = Visibility::Hidden;
            pools.volume_bars.return_entity(entity, PooledEntityType::VolumeBar);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
        }
        return;
    }

    // Calculate max volume from aggregated data for proper Y-scaling
    let max_aggregated_volume = candles_to_render
        .iter()
        .map(|c| c.volume as f32)
        .fold(0.0f32, f32::max) * config.volume_y_axis_padding;

    // Collect existing entities by candle_index
    use std::collections::{HashMap, HashSet};
    let mut existing_bars: HashMap<usize, Entity> = HashMap::new();

    // Only collect existing entities if aggregation level hasn't changed
    // If level changed, all entities were just cleared and have stale indices
    if level == agg_state.previous_level {
        for (entity, bar, _, _, _) in query.iter() {
            existing_bars.insert(bar.candle_index, entity);
        }
    }

    // Track which entities we updated
    let mut updated_bars = HashSet::new();
    let mut spawned_count = 0;

    // Process each visible candle
    for (idx, candle) in candles_to_render.iter().enumerate() {
        // Map aggregated index to original world space
        let i = if level != AggregationLevel::None {
            // Each aggregated candle represents a group of `ratio` source candles.
            // Position at the CENTER of the group for visual accuracy (see render_candlesticks for details).
            start + (idx * level.ratio()) + level.center_offset()
        } else {
            index_mapping_offset + idx
        };

        // Calculate positions using ChartSpace with custom Y-axis bounds for aggregated volumes
        // Use max_aggregated_volume as the custom Y range (0.0 to max_aggregated_volume)
        let bar_bottom = volume_pane.space.to_world_with_y_range(
            i,
            0.0,
            chart.visible_candle_start,
            chart.visible_candle_count,
            0.0,
            max_aggregated_volume,
        );
        let bar_top = volume_pane.space.to_world_with_y_range(
            i,
            candle.volume as f32,
            chart.visible_candle_start,
            chart.visible_candle_count,
            0.0,
            max_aggregated_volume,
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
            Color::srgba(0.0, 0.8, 0.2, 0.6) // Green with transparency
        } else {
            Color::srgba(0.9, 0.2, 0.2, 0.6) // Red with transparency
        };

        // UPDATE or GET FROM POOL volume bar entity
        if let Some(&entity) = existing_bars.get(&i) {
            // Update existing bar
            if let Ok((_, _, mut transform, mut sprite, mut visibility)) = query.get_mut(entity) {
                transform.translation = bar_center.extend(0.0);
                sprite.color = bar_color;
                if let Some(ref mut size) = sprite.custom_size {
                    *size = Vec2::new(bar_width, bar_height);
                }
                *visibility = Visibility::Visible;
            }
            updated_bars.insert(i);
        } else if let Some(entity) = pools.volume_bars.get() {
            // Reuse from pool
            if let Ok((_, mut bar, mut transform, mut sprite, mut visibility)) =
                query.get_mut(entity)
            {
                bar.candle_index = i;
                transform.translation = bar_center.extend(0.0);
                sprite.color = bar_color;
                if let Some(ref mut size) = sprite.custom_size {
                    *size = Vec2::new(bar_width, bar_height);
                }
                *visibility = Visibility::Visible;
                if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                    pooled.in_use = true;
                }
                updated_bars.insert(i);
            } else {
                // Return entity to pool if query failed
                pools.volume_bars.return_entity(entity, PooledEntityType::VolumeBar);
            }
        } else {
            // Pool exhausted - spawn new
            let new_entity = commands
                .spawn((
                    Sprite {
                        color: bar_color,
                        custom_size: Some(Vec2::new(bar_width, bar_height)),
                        ..default()
                    },
                    Transform::from_translation(bar_center.extend(0.0)),
                    VolumeBar { candle_index: i },
                    PooledEntity {
                        entity_type: PooledEntityType::VolumeBar,
                        in_use: true,
                    },
                    VolumeElement,
                    PaneId::Volume,
                ))
                .id();
            pools.volume_bars.add_entity(new_entity);
            spawned_count += 1;
        }
    }

    // Hide and return to pool entities that are no longer visible
    let mut hidden_count = 0;
    for (index, entity) in existing_bars.iter() {
        if !updated_bars.contains(index) {
            if let Ok((_, _, _, _, mut visibility)) = query.get_mut(*entity) {
                *visibility = Visibility::Hidden;
                pools.volume_bars.return_entity(*entity, PooledEntityType::VolumeBar);
                if let Ok(mut pooled) = pooled_query.get_mut(*entity) {
                    pooled.in_use = false;
                }
                hidden_count += 1;
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        let (vol_total, vol_avail) = pools.volume_bars.stats();
        println!(
            "Rendered {} volume bars (indices {}-{}), updated: {}, spawned: {}, hidden: {}",
            end - start,
            start,
            end - 1,
            updated_bars.len(),
            spawned_count,
            hidden_count
        );
        println!("Volume pool stats: {}/{}", vol_avail, vol_total);
    }
}

/// Initialize persistent crosshair entities (called once at startup from setup)
pub fn init_crosshair(commands: &mut Commands, chart: &Chart) {
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
                    color: Color::srgba(1.0, 1.0, 1.0, 0.6),
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
                        color: Color::srgba(1.0, 1.0, 1.0, 0.6),
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
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
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
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
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
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
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

pub fn render_grid_and_axes(
    mut commands: Commands,
    chart: Res<Chart>,
    grid: Res<ChartGrid>,
    axes: Res<ChartAxes>,
    config: Res<CandlestickLODConfig>,
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
                GridElement,
            ));

            // Y-axis label on the right side
            if axes.show_y_labels {
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
            Transform::from_translation(Vec3::new(viewport.center().x, viewport.max.y, 0.4)),
            GridElement,
        ));

        // Bottom border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(viewport.width(), border_thickness)),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.center().x, viewport.min.y, 0.4)),
            GridElement,
        ));

        // Left border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.min.x, viewport.center().y, 0.4)),
            GridElement,
        ));

        // Right border
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(border_thickness, viewport.height())),
                ..default()
            },
            Transform::from_translation(Vec3::new(viewport.max.x, viewport.center().y, 0.4)),
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
            let candle_index = chart.visible_candle_start
                + (candle_percent * chart.visible_candle_count as f32) as usize;

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
            let candle_index = chart.visible_candle_start
                + (candle_percent * chart.visible_candle_count as f32) as usize;

            if candle_index >= chart.candles.len() {
                continue;
            }

            let x = chart_left + candle_percent * (chart_right - chart_left);
            let candle = &chart.candles[candle_index];
            let label_y = chart_bottom - 40.0;

            // Format timestamp using chrono
            let datetime =
                DateTime::<Utc>::from_timestamp(candle.time / 1000, 0).unwrap_or_default();
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
    let first_pane = &chart.panes[0];
    let chart_right = first_pane.space.viewport.max.x;

    let mouse_x = interaction.mouse_pos.x;
    let mouse_y = interaction.mouse_pos.y;

    // ========== OPTIMIZATION: Only update positions if mouse moved ==========
    let mouse_moved = interaction.mouse_pos != interaction.last_crosshair_mouse_pos;

    if mouse_moved {
        interaction.last_crosshair_mouse_pos = interaction.mouse_pos;

        // ========== UPDATE VERTICAL CROSSHAIR LINE POSITIONS ==========
        for segment in &crosshair_entities.vertical_line_segments {
            if let Ok(mut transform) = transforms.get_mut(*segment) {
                transform.translation.x = mouse_x;
            }
            // ✅ Visibility already set by state change handler above
        }

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
                            chart.visible_candle_start,
                            chart.visible_candle_count,
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

    // ========== FIND CANDLE AT CURSOR ==========
    let (candle_index, _) = if let Some(pane) = chart.panes.first() {
        pane.space.from_world(
            interaction.mouse_pos,
            chart.visible_candle_start,
            chart.visible_candle_count,
        )
    } else {
        return;
    };

    if candle_index >= chart.candles.len() {
        interaction.last_crosshair_candle_index = None;
        return;
    }

    // ========== DEBOUNCE: Only update text if candle changed ==========
    let candle_changed = interaction.last_crosshair_candle_index != Some(candle_index);

    if candle_changed {
        interaction.last_crosshair_candle_index = Some(candle_index);
        let candle = &chart.candles[candle_index];

        // ========== UPDATE TIME LABEL TEXT (only when candle changes) ==========
        if crosshair.show_time_label {
            let datetime =
                DateTime::<Utc>::from_timestamp(candle.time / 1000, 0).unwrap_or_default();
            let label_text = datetime.format("%m/%d %H:%M").to_string();

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

    // ========== UPDATE TIME LABEL POSITION (when mouse moves) ==========
    if mouse_moved && crosshair.show_time_label {
        if let Ok(mut transform) = transforms.get_mut(crosshair_entities.time_label) {
            transform.translation.x = mouse_x;
        }
        // ✅ Visibility already set by state change handler above
    }
}

pub fn render_moving_averages(mut gizmos: Gizmos, chart: Res<Chart>) {
    // Find the Price pane (indicators overlay on price)
    let price_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price));

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
            if let (Some(curr_value), Some(next_value)) =
                (ma.values[i], ma.values.get(i + 1).and_then(|v| *v))
            {
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
