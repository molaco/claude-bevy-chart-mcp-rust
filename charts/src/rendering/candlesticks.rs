use crate::cache::RenderCache;
use crate::types::*;
use crate::aggregation::*;
use crate::theme::ChartColors;
use bevy::prelude::*;

pub fn render_candlesticks(
    mut commands: Commands,
    chart: Res<Chart>,
    config: Res<CandlestickLODConfig>,
    colors: Res<ChartColors>,
    agg_config: Res<AggregationConfig>,
    mut agg_cache: ResMut<AggregationCache>,
    agg_state: Res<AggregationState>,  // Read-only: level detection done in detect_aggregation_level_change
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
    if !chart.needs_redraw || chart.load_status != ChartLoadStatus::Ready {
        return;
    }

    let price_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price));
    if price_pane.is_none() {
        return;
    }
    let price_pane = price_pane.unwrap();

    let time_start = chart.visible_time_start;
    let time_end = chart.visible_time_end;

    // Count visible candles for LOD calculation
    let visible_count = chart.candles.range(time_start..=time_end).count();
    if visible_count == 0 {
        return;
    }

    // Determine LOD level based on candle width
    let candle_width_px = price_pane.space.viewport.width() / visible_count as f32;
    let lod_level = calculate_lod_level(candle_width_px, &config);

    // Use pre-computed aggregation level from detect_aggregation_level_change system
    // This ensures all rendering systems see the same level (no race condition)
    let level = agg_state.current_level;

    // Check if pools need to be cleared (level changed this frame)
    // The detection system already set level_changed_this_frame if level changed
    let pools_just_cleared = agg_state.level_changed_this_frame;

    if pools_just_cleared {
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

    // Collect existing entities by candle_timestamp
    use std::collections::HashMap;
    let mut existing_wicks: HashMap<i64, Entity> = HashMap::new();
    let mut existing_bodies: HashMap<i64, Entity> = HashMap::new();
    let mut existing_ohlc: HashMap<i64, Entity> = HashMap::new();
    let mut existing_range: HashMap<i64, Entity> = HashMap::new();

    // Only collect existing entities if aggregation level hasn't changed
    // If level changed, all entities were just cleared and have stale timestamps
    if !pools_just_cleared {
        for (entity, wick, _, _, _) in query_wicks.iter() {
            existing_wicks.insert(wick.candle_timestamp, entity);
        }
        for (entity, body, _, _, _) in query_bodies.iter() {
            existing_bodies.insert(body.candle_timestamp, entity);
        }
        for (entity, ohlc, _, _, _) in query_ohlc.iter() {
            existing_ohlc.insert(ohlc.candle_timestamp, entity);
        }
        for (entity, range_line, _, _, _) in query_range.iter() {
            existing_range.insert(range_line.candle_timestamp, entity);
        }
    }

    // Track which entities we updated (to avoid despawning them)
    // Use HashSet<Entity> instead of HashSet<usize> to avoid stale index issues during panning
    let mut updated_wicks = std::collections::HashSet::<Entity>::new();
    let mut updated_bodies = std::collections::HashSet::<Entity>::new();
    let mut updated_ohlc = std::collections::HashSet::<Entity>::new();
    let mut updated_range = std::collections::HashSet::<Entity>::new();

    // Counters for debug output
    let mut spawned_count = 0;

    // Track rendering operations for gap detection
    #[cfg(debug_assertions)]
    let mut rendered_timestamps = Vec::new();
    #[cfg(debug_assertions)]
    let mut pool_get_success = 0;
    #[cfg(debug_assertions)]
    let mut pool_get_failed_query = 0;
    #[cfg(debug_assertions)]
    let mut pool_exhausted = 0;
    #[cfg(debug_assertions)]
    let mut entity_reused = 0;

    // DIRECT ITERATION - no Vec allocation
    for (timestamp, candle) in chart.candles.range(time_start..=time_end) {
        let ts = *timestamp;

        #[cfg(debug_assertions)]
        rendered_timestamps.push(ts);

        match lod_level {
            CandleLODLevel::Full => {
                // Full detail: render wick + body (hide LOD entities if they exist)
                if let Some(&entity) = existing_ohlc.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_ohlc.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.ohlc_lines.return_entity(entity, PooledEntityType::CandlestickOHLC);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_range.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_range.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.range_lines.return_entity(entity, PooledEntityType::CandlestickRange);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }

                // Calculate positions using ChartSpace::to_world()
                let wick_bottom = price_pane.space.to_world(ts, candle.low as f32, time_start, time_end);
                let wick_top = price_pane.space.to_world(ts, candle.high as f32, time_start, time_end);
                let body_open = price_pane.space.to_world(ts, candle.open as f32, time_start, time_end);
                let body_close = price_pane.space.to_world(ts, candle.close as f32, time_start, time_end);

                let wick_center = Vec2::new(
                    (wick_bottom.x + wick_top.x) / 2.0,
                    (wick_bottom.y + wick_top.y) / 2.0,
                );
                let wick_height = (wick_top.y - wick_bottom.y).abs().max(1.0);

                // UPDATE or GET FROM POOL wick entity
                if let Some(&entity) = existing_wicks.get(&ts) {
                    #[cfg(debug_assertions)]
                    { entity_reused += 1; }

                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_wicks.get_mut(entity)
                    {
                        transform.translation = wick_center.extend(0.0);
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.0, wick_height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_wicks.insert(entity);
                } else if let Some(entity) = pools.wicks.get() {
                    #[cfg(debug_assertions)]
                    { pool_get_success += 1; }
                    // Reuse from pool
                    if let Ok((_, mut wick, mut transform, mut sprite, mut visibility)) =
                        query_wicks.get_mut(entity)
                    {
                        wick.candle_timestamp = ts;
                        transform.translation = wick_center.extend(0.0);
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.0, wick_height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_wicks.insert(entity);
                    } else {
                        #[cfg(debug_assertions)]
                        { pool_get_failed_query += 1; }

                        // Entity from pool is invalid - remove it permanently and spawn new as fallback
                        pools.wicks.remove_invalid(entity);

                        // Spawn new entity to replace the corrupted one
                        // NOTE: Don't add to pool yet - entity doesn't exist until next frame!
                        // It will be auto-discovered when needed.
                        let new_entity = commands
                            .spawn((
                                Sprite {
                                    color: colors.text,
                                    custom_size: Some(Vec2::new(1.0, wick_height)),
                                    ..default()
                                },
                                Transform::from_translation(wick_center.extend(0.0)),
                                CandlestickWick { candle_timestamp: ts },
                                PooledEntity {
                                    entity_type: PooledEntityType::CandlestickWick,
                                    in_use: true,
                                },
                                PendingPoolEntry {
                                    entity_type: PooledEntityType::CandlestickWick,
                                },
                                PriceElement,
                                PaneId::Price,
                            ))
                            .id();
                        // pools.wicks.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                        updated_wicks.insert(new_entity);
                        spawned_count += 1;
                    }
                } else {
                    #[cfg(debug_assertions)]
                    { pool_exhausted += 1; }

                    // Pool exhausted - spawn new
                    // NOTE: Don't add to pool yet - entity doesn't exist until next frame!
                    let new_entity = commands
                        .spawn((
                            Sprite {
                                color: colors.text,
                                custom_size: Some(Vec2::new(1.0, wick_height)),
                                ..default()
                            },
                            Transform::from_translation(wick_center.extend(0.0)),
                            CandlestickWick { candle_timestamp: ts },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickWick,
                                in_use: true,
                            },
                            PendingPoolEntry {
                                entity_type: PooledEntityType::CandlestickWick,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    // pools.wicks.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                    updated_wicks.insert(new_entity);
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
                    colors.bull_candle
                } else {
                    colors.bear_candle
                };

                // UPDATE or GET FROM POOL body entity
                if let Some(&entity) = existing_bodies.get(&ts) {
                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_bodies.get_mut(entity)
                    {
                        transform.translation = body_center.extend(1.0);
                        // Only update color if it changed to prevent unnecessary change detection
                        if sprite.color != body_color {
                            sprite.color = body_color;
                        }
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(body_width, body_height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_bodies.insert(entity);
                } else if let Some(entity) = pools.bodies.get() {
                    // Reuse from pool
                    if let Ok((_, mut body, mut transform, mut sprite, mut visibility)) =
                        query_bodies.get_mut(entity)
                    {
                        body.candle_timestamp = ts;
                        transform.translation = body_center.extend(1.0);
                        sprite.color = body_color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(body_width, body_height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_bodies.insert(entity);
                    } else {
                        // Entity from pool is invalid - remove it permanently and spawn new as fallback
                        pools.bodies.remove_invalid(entity);

                        // Spawn new entity to replace the corrupted one
                        let new_entity = commands
                            .spawn((
                                Sprite {
                                    color: body_color,
                                    custom_size: Some(Vec2::new(body_width, body_height)),
                                    ..default()
                                },
                                Transform::from_translation(body_center.extend(1.0)),
                                CandlestickBody { candle_timestamp: ts },
                                PooledEntity {
                                    entity_type: PooledEntityType::CandlestickBody,
                                    in_use: true,
                                },
                                PendingPoolEntry {
                                    entity_type: PooledEntityType::CandlestickBody,
                                },
                                PriceElement,
                                PaneId::Price,
                            ))
                            .id();
                        // pools.bodies.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                        updated_bodies.insert(new_entity);
                        spawned_count += 1;
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
                            CandlestickBody { candle_timestamp: ts },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickBody,
                                in_use: true,
                            },
                            PendingPoolEntry {
                                entity_type: PooledEntityType::CandlestickBody,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    // pools.bodies.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                    updated_bodies.insert(new_entity);
                    spawned_count += 1;
                }
            }

            CandleLODLevel::Medium => {
                // Medium detail: single OHLC line (hide full detail entities if they exist)
                if let Some(&entity) = existing_wicks.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_wicks.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.wicks.return_entity(entity, PooledEntityType::CandlestickWick);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_bodies.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_bodies.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.bodies.return_entity(entity, PooledEntityType::CandlestickBody);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_range.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_range.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.range_lines.return_entity(entity, PooledEntityType::CandlestickRange);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }

                let low_pos = price_pane.space.to_world(ts, candle.low as f32, time_start, time_end);
                let high_pos = price_pane.space.to_world(ts, candle.high as f32, time_start, time_end);

                let center = Vec2::new(
                    (low_pos.x + high_pos.x) / 2.0,
                    (low_pos.y + high_pos.y) / 2.0,
                );
                let height = (high_pos.y - low_pos.y).abs().max(1.0);

                let color = if candle.close >= candle.open {
                    colors.bull_candle
                } else {
                    colors.bear_candle
                };

                // UPDATE or GET FROM POOL OHLC line entity
                if let Some(&entity) = existing_ohlc.get(&ts) {
                    #[cfg(debug_assertions)]
                    { entity_reused += 1; }

                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_ohlc.get_mut(entity)
                    {
                        transform.translation = center.extend(0.0);
                        // Only update color if it changed to prevent unnecessary change detection
                        if sprite.color != color {
                            sprite.color = color;
                        }
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.5, height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_ohlc.insert(entity);
                } else if let Some(entity) = pools.ohlc_lines.get() {
                    #[cfg(debug_assertions)]
                    { pool_get_success += 1; }

                    // Reuse from pool
                    if let Ok((_, mut ohlc, mut transform, mut sprite, mut visibility)) =
                        query_ohlc.get_mut(entity)
                    {
                        ohlc.candle_timestamp = ts;
                        transform.translation = center.extend(0.0);
                        sprite.color = color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(1.5, height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_ohlc.insert(entity);
                    } else {
                        #[cfg(debug_assertions)]
                        { pool_get_failed_query += 1; }

                        // Entity from pool is invalid - remove it permanently and spawn new as fallback
                        pools.ohlc_lines.remove_invalid(entity);

                        // Spawn new entity to replace the corrupted one
                        let new_entity = commands
                            .spawn((
                                Sprite {
                                    color,
                                    custom_size: Some(Vec2::new(1.5, height)),
                                    ..default()
                                },
                                Transform::from_translation(center.extend(0.0)),
                                CandlestickOHLCLine { candle_timestamp: ts },
                                PooledEntity {
                                    entity_type: PooledEntityType::CandlestickOHLC,
                                    in_use: true,
                                },
                                PendingPoolEntry {
                                    entity_type: PooledEntityType::CandlestickOHLC,
                                },
                                PriceElement,
                                PaneId::Price,
                            ))
                            .id();
                        // pools.ohlc_lines.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                        updated_ohlc.insert(new_entity);
                        spawned_count += 1;
                    }
                } else {
                    #[cfg(debug_assertions)]
                    { pool_exhausted += 1; }

                    // Pool exhausted - spawn new
                    let new_entity = commands
                        .spawn((
                            Sprite {
                                color,
                                custom_size: Some(Vec2::new(1.5, height)),
                                ..default()
                            },
                            Transform::from_translation(center.extend(0.0)),
                            CandlestickOHLCLine { candle_timestamp: ts },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickOHLC,
                                in_use: true,
                            },
                            PendingPoolEntry {
                                entity_type: PooledEntityType::CandlestickOHLC,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    // pools.ohlc_lines.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                    updated_ohlc.insert(new_entity);
                    spawned_count += 1;
                }
            }

            CandleLODLevel::Low => {
                // Low detail: single range line (hide other entities if they exist)
                if let Some(&entity) = existing_wicks.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_wicks.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.wicks.return_entity(entity, PooledEntityType::CandlestickWick);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_bodies.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_bodies.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.bodies.return_entity(entity, PooledEntityType::CandlestickBody);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }
                if let Some(&entity) = existing_ohlc.get(&ts) {
                    if let Ok((_, _, _, _, mut visibility)) = query_ohlc.get_mut(entity) {
                        *visibility = Visibility::Hidden;
                        pools.ohlc_lines.return_entity(entity, PooledEntityType::CandlestickOHLC);
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = false;
                        }
                    }
                }

                let low_pos = price_pane.space.to_world(ts, candle.low as f32, time_start, time_end);
                let high_pos = price_pane.space.to_world(ts, candle.high as f32, time_start, time_end);

                let center = Vec2::new(
                    (low_pos.x + high_pos.x) / 2.0,
                    (low_pos.y + high_pos.y) / 2.0,
                );
                let height = (high_pos.y - low_pos.y).abs().max(1.0);

                let color = if candle.close >= candle.open {
                    colors.bull_candle
                } else {
                    colors.bear_candle
                };

                // UPDATE or GET FROM POOL range line entity
                if let Some(&entity) = existing_range.get(&ts) {
                    #[cfg(debug_assertions)]
                    { entity_reused += 1; }

                    if let Ok((_, _, mut transform, mut sprite, mut visibility)) =
                        query_range.get_mut(entity)
                    {
                        transform.translation = center.extend(0.0);
                        // Only update color if it changed to prevent unnecessary change detection
                        if sprite.color != color {
                            sprite.color = color;
                        }
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(0.5, height);
                        }
                        *visibility = Visibility::Visible;
                    }
                    updated_range.insert(entity);
                } else if let Some(entity) = pools.range_lines.get() {
                    #[cfg(debug_assertions)]
                    { pool_get_success += 1; }

                    // Reuse from pool
                    if let Ok((_, mut range, mut transform, mut sprite, mut visibility)) =
                        query_range.get_mut(entity)
                    {
                        range.candle_timestamp = ts;
                        transform.translation = center.extend(0.0);
                        sprite.color = color;
                        if let Some(ref mut size) = sprite.custom_size {
                            *size = Vec2::new(0.5, height);
                        }
                        *visibility = Visibility::Visible;
                        if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                            pooled.in_use = true;
                        }
                        updated_range.insert(entity);
                    } else {
                        #[cfg(debug_assertions)]
                        { pool_get_failed_query += 1; }

                        // Entity from pool is invalid - remove it permanently and spawn new as fallback
                        pools.range_lines.remove_invalid(entity);

                        // Spawn new entity to replace the corrupted one
                        let new_entity = commands
                            .spawn((
                                Sprite {
                                    color,
                                    custom_size: Some(Vec2::new(0.5, height)),
                                    ..default()
                                },
                                Transform::from_translation(center.extend(0.0)),
                                CandlestickRangeLine { candle_timestamp: ts },
                                PooledEntity {
                                    entity_type: PooledEntityType::CandlestickRange,
                                    in_use: true,
                                },
                                PendingPoolEntry {
                                    entity_type: PooledEntityType::CandlestickRange,
                                },
                                PriceElement,
                                PaneId::Price,
                            ))
                            .id();
                        // pools.range_lines.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                        updated_range.insert(new_entity);
                        spawned_count += 1;
                    }
                } else {
                    #[cfg(debug_assertions)]
                    { pool_exhausted += 1; }

                    // Pool exhausted - spawn new
                    let new_entity = commands
                        .spawn((
                            Sprite {
                                color,
                                custom_size: Some(Vec2::new(0.5, height)),
                                ..default()
                            },
                            Transform::from_translation(center.extend(0.0)),
                            CandlestickRangeLine { candle_timestamp: ts },
                            PooledEntity {
                                entity_type: PooledEntityType::CandlestickRange,
                                in_use: true,
                            },
                            PendingPoolEntry {
                                entity_type: PooledEntityType::CandlestickRange,
                            },
                            PriceElement,
                            PaneId::Price,
                        ))
                        .id();
                    // pools.range_lines.add_entity(new_entity);  // DON'T ADD - causes pool corruption!
                    updated_range.insert(new_entity);
                    spawned_count += 1;
                }
            }
        }
    }

    // Hide and return to pool entities that are no longer visible
    // Iterate through all entities (not HashMap) to avoid stale index issues
    let mut hidden_count = 0;
    for (entity, _, _, _, mut visibility) in query_wicks.iter_mut() {
        if !updated_wicks.contains(&entity) {
            *visibility = Visibility::Hidden;
            pools.wicks.return_entity(entity, PooledEntityType::CandlestickWick);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
            hidden_count += 1;
        }
    }
    for (entity, _, _, _, mut visibility) in query_bodies.iter_mut() {
        if !updated_bodies.contains(&entity) {
            *visibility = Visibility::Hidden;
            pools.bodies.return_entity(entity, PooledEntityType::CandlestickBody);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
            hidden_count += 1;
        }
    }
    for (entity, _, _, _, mut visibility) in query_ohlc.iter_mut() {
        if !updated_ohlc.contains(&entity) {
            *visibility = Visibility::Hidden;
            pools.ohlc_lines.return_entity(entity, PooledEntityType::CandlestickOHLC);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
            hidden_count += 1;
        }
    }
    for (entity, _, _, _, mut visibility) in query_range.iter_mut() {
        if !updated_range.contains(&entity) {
            *visibility = Visibility::Hidden;
            pools.range_lines.return_entity(entity, PooledEntityType::CandlestickRange);
            if let Ok(mut pooled) = pooled_query.get_mut(entity) {
                pooled.in_use = false;
            }
            hidden_count += 1;
        }
    }

    #[cfg(debug_assertions)]
    {
        let stats = agg_cache.stats();
        let (wick_total, wick_avail) = pools.wicks.stats();
        let (body_total, body_avail) = pools.bodies.stats();

        let mut log_msg = format!(
            "CANDLES: Rendered {} | Cache: {:.1}% hit ({}/{}) | Pools: W:{}/{} B:{}/{}",
            visible_count,
            stats.hit_rate() * 100.0,
            stats.hits,
            stats.hits + stats.misses,
            wick_total - wick_avail,
            wick_total,
            body_total - body_avail,
            body_total,
        );

        if pools_just_cleared {
            log_msg.push_str(" | POOLS CLEARED");
        }
        if spawned_count > 0 {
            log_msg.push_str(&format!(" | Spawned: {}", spawned_count));
        }
        if hidden_count > 0 {
            log_msg.push_str(&format!(" | Hidden: {}", hidden_count));
        }

        println!("{}", log_msg);

        // Detailed entity lifecycle logging
        println!(
            "  Entity Ops: Reused={} | PoolOK={} | PoolFailed={} | PoolEmpty={} | Total={}",
            entity_reused,
            pool_get_success,
            pool_get_failed_query,
            pool_exhausted,
            entity_reused + pool_get_success + pool_get_failed_query + pool_exhausted
        );

        // Timestamp gap detection
        if !rendered_timestamps.is_empty() {
            rendered_timestamps.sort_unstable();
            println!(
                "  Time range: {} - {} ({} candles)",
                rendered_timestamps.first().unwrap_or(&0),
                rendered_timestamps.last().unwrap_or(&0),
                rendered_timestamps.len()
            );
        }
    }
}
