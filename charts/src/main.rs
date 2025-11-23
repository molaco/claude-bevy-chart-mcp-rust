mod aggregation;
mod api;
mod cache;
mod error;
mod focus;
mod interaction;
mod realtime;
mod rendering;
mod screenshot;
mod theme;
mod types;
mod ui_layout;
mod websocket;

use anyhow::{Context, Result};
use bevy::camera::Camera2d;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy::remote::{BrpResult, RemotePlugin};
use bevy::time::common_conditions::on_timer;
use bevy::window::PresentMode;
use chat_ui::prelude::*;
use chrono::NaiveDate;
use clap::Parser;
use focus::*;
use interaction::*;
use rendering::*;
use screenshot::{crop_screenshot_to_chart, take_screenshot};
use serde_json::Value;
use std::time::Duration;
use types::*;
// use ui_layout::ChartViewport;

// ============================================================================
// CLI ARGUMENTS
// ============================================================================

#[derive(Parser, Debug, Clone, Resource)]
#[command(name = "charts")]
#[command(about = "Display candlestick charts from DuckDB database", long_about = None)]
struct ChartArgs {
    /// Trading pair symbol (e.g., BTCUSDT, ETHUSDT)
    #[arg(short, long)]
    ticker: String,

    /// Timeframe/interval (e.g., 1m, 5m, 15m, 1h, 4h, 1d)
    #[arg(short = 'i', long)]
    timeframe: String,

    /// Start date (YYYY-MM-DD format)
    #[arg(short, long)]
    start: String,

    /// End date (YYYY-MM-DD format)
    #[arg(short, long)]
    end: String,

    /// Database path (optional, defaults to flowsurface database)
    #[arg(
        long,
        default_value = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb"
    )]
    db_path: String,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Resolve ticker symbol to ticker_id from database
fn resolve_ticker_id(db: &ChartDatabase, symbol: &str) -> Result<i32> {
    db.resolve_ticker(symbol)
        .with_context(|| format!(
            "Ticker '{}' not found in database. Available tickers can be listed with: SELECT symbol FROM tickers",
            symbol
        ))
}

/// Parse date string (YYYY-MM-DD) to millisecond timestamp
fn parse_date_to_timestamp(date_str: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .with_context(|| format!("Invalid date format: '{}'. Expected YYYY-MM-DD", date_str))?;

    // Convert to midnight UTC timestamp in milliseconds
    let timestamp = date
        .and_hms_opt(0, 0, 0)
        .expect("Valid time")
        .and_utc()
        .timestamp_millis();

    Ok(timestamp)
}

/// Validate that data exists for the given parameters
fn validate_data_exists(
    db: &ChartDatabase,
    ticker_id: i32,
    timeframe: &str,
    start_time: i64,
    end_time: i64,
    ticker_symbol: &str,
) -> Result<usize> {
    let count = db
        .check_data_exists(ticker_id, timeframe, start_time, end_time)
        .context("Failed to check data existence")?;

    if count == 0 {
        anyhow::bail!(
            "No klines found for {} {} from {} to {}.\n\
             Try importing data first:\n  \
             cargo run --bin data-loader -- klines -t {} -i {} -s {} -e {}",
            ticker_symbol,
            timeframe,
            chrono::DateTime::from_timestamp_millis(start_time)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| start_time.to_string()),
            chrono::DateTime::from_timestamp_millis(end_time)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| end_time.to_string()),
            ticker_symbol,
            timeframe,
            chrono::DateTime::from_timestamp_millis(start_time)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| start_time.to_string()),
            chrono::DateTime::from_timestamp_millis(end_time)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| end_time.to_string()),
        );
    }

    Ok(count)
}

// ============================================================================
// CHART CONTEXT UPDATE
// ============================================================================

/// System that updates the chat context with current chart state
fn update_chart_context(chart: Res<Chart>, mut chat_state: ResMut<ChatState>) {
    // Only update if chat is waiting for input (user is about to send a message)
    // or if chart has changed significantly
    if chat_state.is_changed() || chart.is_changed() {
        let context = format!(
            "Chart: {} candles visible (time {}-{}), price range: ${:.2}-${:.2}, {} indicators active, volume pane: {}",
            chart.visible_candle_count(),
            chart.visible_time_start,
            chart.visible_time_end,
            chart.panes.get(0).map_or(0.0, |p| p.space.visible_price_min),
            chart.panes.get(0).map_or(0.0, |p| p.space.visible_price_max),
            chart.indicators.iter().filter(|i| i.visible).count(),
            if chart.panes.len() > 1 { "visible" } else { "hidden" }
        );
        chat_state.chart_context = Some(context);
    }
}

// ============================================================================
// ENTITY POOL HEALTH MONITORING
// ============================================================================

/// Cleanup system to detect and remove zombie entities from pools
fn cleanup_entity_pools(mut pools: ResMut<EntityPools>, query: Query<Entity, With<PooledEntity>>) {
    let valid_entities: std::collections::HashSet<Entity> = query.iter().collect();

    let mut removed_total = 0;

    // Clean wicks pool
    let removed = cleanup_pool(&mut pools.wicks, &valid_entities);
    removed_total += removed;

    // Clean bodies pool
    let removed = cleanup_pool(&mut pools.bodies, &valid_entities);
    removed_total += removed;

    // Clean OHLC pool
    let removed = cleanup_pool(&mut pools.ohlc_lines, &valid_entities);
    removed_total += removed;

    // Clean range pool
    let removed = cleanup_pool(&mut pools.range_lines, &valid_entities);
    removed_total += removed;

    // Clean volume pool
    let removed = cleanup_pool(&mut pools.volume_bars, &valid_entities);
    removed_total += removed;

    if removed_total > 0 {
        println!("Cleaned {} zombie entities from pools", removed_total);
    }
}

/// Helper function to cleanup a single pool
fn cleanup_pool(
    pool: &mut EntityPool,
    valid_entities: &std::collections::HashSet<Entity>,
) -> usize {
    let initial_count = pool.all_entities.len();

    // Remove invalid entities from all_entities
    pool.all_entities
        .retain(|entity| valid_entities.contains(entity));

    // Remove invalid entities from available
    pool.available
        .retain(|entity| valid_entities.contains(entity));

    initial_count - pool.all_entities.len()
}

/// Shrink pools that have grown beyond initial size
fn shrink_entity_pools(
    mut commands: Commands,
    mut pools: ResMut<EntityPools>,
    config: Res<EntityPoolConfig>,
    query: Query<Entity, (With<PooledEntity>, Without<Visibility>)>,
) {
    if !config.enabled {
        return;
    }

    let target_size = PoolSizeConfig::default().initial_pool_size;

    // Shrink each pool
    shrink_pool(
        &mut commands,
        &mut pools.wicks,
        target_size,
        &query,
        "Wicks",
    );
    shrink_pool(
        &mut commands,
        &mut pools.bodies,
        target_size,
        &query,
        "Bodies",
    );
    shrink_pool(
        &mut commands,
        &mut pools.ohlc_lines,
        target_size,
        &query,
        "OHLC",
    );
    shrink_pool(
        &mut commands,
        &mut pools.range_lines,
        target_size,
        &query,
        "Range",
    );
    shrink_pool(
        &mut commands,
        &mut pools.volume_bars,
        target_size,
        &query,
        "Volume",
    );
}

/// Helper function to shrink a single pool
fn shrink_pool(
    commands: &mut Commands,
    pool: &mut EntityPool,
    target_size: usize,
    query: &Query<Entity, (With<PooledEntity>, Without<Visibility>)>,
    pool_name: &str,
) {
    let current_size = pool.all_entities.len();

    if current_size <= target_size {
        return;
    }

    let excess = current_size - target_size;
    let to_remove = excess.min(50); // Remove max 50 entities per cycle

    let mut removed = 0;

    // Only remove available (unused) entities
    let available_entities: Vec<Entity> = pool
        .available
        .iter()
        .copied()
        .take(to_remove)
        .collect();

    for entity in available_entities.iter() {
        // Verify entity is actually unused (hidden/off-screen)
        if query.contains(*entity) {
            commands.entity(*entity).despawn();
            pool.all_entities.remove(entity);
            pool.available.remove(entity);
            removed += 1;
        }
    }

    if removed > 0 {
        println!(
            "Shrunk {} pool: removed {} entities ({} -> {})",
            pool_name,
            removed,
            current_size,
            pool.all_entities.len()
        );
    }
}

/// Discover newly spawned entities and add them to pools
/// Runs after spawn commands have been applied, so entities actually exist
fn discover_and_pool_entities(
    query: Query<(Entity, &PendingPoolEntry)>,
    mut pools: ResMut<EntityPools>,
    config: Res<EntityPoolConfig>,
    chart: Res<Chart>,
    mut commands: Commands,
) {
    for (entity, pending) in query.iter() {
        let pool_config = config.for_timeframe(&chart.timeframe);
        let max_size = pool_config.max_pool_size;

        let added = match pending.entity_type {
            PooledEntityType::CandlestickWick => {
                pools.wicks.try_add_entity(entity, max_size)
            }
            PooledEntityType::CandlestickBody => {
                pools.bodies.try_add_entity(entity, max_size)
            }
            PooledEntityType::CandlestickOHLC => {
                pools.ohlc_lines.try_add_entity(entity, max_size)
            }
            PooledEntityType::CandlestickRange => {
                pools.range_lines.try_add_entity(entity, max_size)
            }
            PooledEntityType::VolumeBar => {
                pools.volume_bars.try_add_entity(entity, max_size)
            }
        };

        // Remove marker component (cleanup)
        if added {
            commands.entity(entity).remove::<PendingPoolEntry>();
        }
        // If not added (pool full), marker stays and we retry next frame
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // Parse CLI arguments
    let args = ChartArgs::parse();

    // Validate arguments before starting Bevy app
    match validate_cli_args(&args) {
        Ok(_) => {
            println!(
                "Loading chart for {} {} from {} to {}",
                args.ticker, args.timeframe, args.start, args.end
            );
        }
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(1);
        }
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Chart: {} {}", args.ticker, args.timeframe),
                resolution: (1600, 900).into(),
                present_mode: PresentMode::AutoNoVsync, // Disable VSync for uncapped FPS
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ChatUiPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(RemotePlugin::default().with_method("chart/screenshot", handle_screenshot)) // Enable Bevy Remote Protocol for MCP integration
        .add_plugins(RemoteHttpPlugin::default()) // Enable HTTP transport on port 15702
        .add_plugins(websocket::WebSocketPlugin)
        .add_plugins(realtime::RealtimePlugin)
        .insert_resource(args) // Inject CLI args as a resource
        .init_resource::<FocusState>()
        .init_resource::<DeferredUpdates>() // Frame-timing: deferred updates for lazy loading
        .insert_resource(EntityPoolConfig::default())
        .insert_resource(ZoomLimitConfig::default())
        .insert_resource(theme::ChartColors::default())
        .insert_resource(cache::RenderCache::new())
        .insert_resource(cache::ThrottleState::default())
        .add_message::<TimeframeChangeRequest>()
        .add_systems(Startup, ui_layout::setup_split_layout)
        .add_systems(Startup, setup)
        .add_systems(Startup, init_entity_pools.after(setup))
        .add_systems(Startup, warmup_aggregation_cache.after(setup))
        .add_systems(Startup, setup_fps_counter)
        .add_systems(Startup, setup_timeframe_label)
        .add_systems(PostStartup, ui_layout::reparent_chat_to_container)
        .add_systems(Update, update_chart_context)
        .add_systems(
            Update,
            (
                handle_focus_tab,
                handle_focus_click,
                update_focus_indicators,
                manage_text_input_focus,
            )
                .chain(),
        )
        // ============================================================================
        // FRAME-TIMING: Apply deferred updates FIRST, before any rendering
        // This ensures data changes from lazy loading are applied at frame start
        // ============================================================================
        .add_systems(Update, reset_aggregation_frame_state) // Reset frame-local flags
        .add_systems(Update, apply_deferred_updates.after(reset_aggregation_frame_state)) // MUST run early!
        .add_systems(Update, toggle_volume_pane.after(apply_deferred_updates))
        .add_systems(Update, toggle_sma_indicators.after(apply_deferred_updates))
        .add_systems(Update, check_lazy_load.after(apply_deferred_updates)) // Queues data for NEXT frame
        .add_systems(Update, screenshot_on_keypress)
        .add_systems(Update, update_fps_counter)
        .add_systems(Update, update_timeframe_label)
        .add_systems(Update, handle_timeframe_keyboard)
        .add_systems(Update, handle_timeframe_change.after(apply_deferred_updates))
        .add_systems(Update, handle_timeframe_change_aggregation.after(handle_timeframe_change))
        .add_systems(
            Update,
            cleanup_entity_pools.run_if(on_timer(Duration::from_secs(10))),
        )
        .add_systems(
            Update,
            shrink_entity_pools.run_if(on_timer(Duration::from_secs(30))),
        )
        .add_systems(Update, (handle_mouse_input, update_crosshair).chain().after(apply_deferred_updates))
        // ============================================================================
        // RENDERING: Runs AFTER all data updates are applied
        // ============================================================================
        .add_systems(
            Update,
            (
                render_grid_and_axes,
                render_candlesticks,
                render_moving_averages,
                render_volume_bars,
                discover_and_pool_entities,
                reset_redraw_flag,
            )
                .chain()
                .after(handle_mouse_input)
                .after(check_lazy_load)
                .after(handle_timeframe_change_aggregation), // Ensure all data updates complete first
        )
        .run();
}

/// Validate CLI arguments and data existence before starting app
fn validate_cli_args(args: &ChartArgs) -> Result<()> {
    // Initialize temporary database connection for validation
    let db = ChartDatabase::new(&args.db_path)
        .with_context(|| format!("Failed to open database at {}", args.db_path))?;

    // Resolve ticker symbol to ID
    let ticker_id = resolve_ticker_id(&db, &args.ticker)?;

    // Parse dates to timestamps
    let start_time = parse_date_to_timestamp(&args.start)?;
    let end_time = parse_date_to_timestamp(&args.end)?;

    // Validate date range
    if end_time < start_time {
        anyhow::bail!("End date must be after start date");
    }

    // Check that data exists
    let count = validate_data_exists(
        &db,
        ticker_id,
        &args.timeframe,
        start_time,
        end_time,
        &args.ticker,
    )?;

    println!("Found {} klines in database", count);

    Ok(())
}

// ============================================================================
// SETUP
// ============================================================================

/// Warm up aggregation cache with common levels for faster initial rendering
fn warmup_aggregation_cache(
    chart: Res<Chart>,
    config: Res<aggregation::AggregationConfig>,
    mut cache: ResMut<aggregation::AggregationCache>,
) {
    if !config.enabled || !config.warmup_on_startup {
        return;
    }

    let start = std::time::Instant::now();

    let timeframe = &chart.timeframe;
    let candles = &chart.candles;
    let time_start = chart.visible_time_start;
    let time_end = chart.visible_time_end;

    // Warm up common levels
    let levels = [
        aggregation::AggregationLevel::None,
        aggregation::AggregationLevel::Low,
        aggregation::AggregationLevel::Medium,
        aggregation::AggregationLevel::High,
    ];

    for level in levels {
        cache.get_or_aggregate(
            timeframe,
            candles,
            time_start,
            time_end,
            level,
        );
    }

    let elapsed = start.elapsed();
    let stats = cache.stats();
    println!(
        "Cache warmed: {} entries, {:.2}MB, took {:?}",
        stats.entries,
        stats.size_bytes as f64 / 1_048_576.0,
        elapsed
    );
}

fn setup(
    mut commands: Commands,
    window_query: Query<(Entity, &Window), With<Window>>,
    args: Res<ChartArgs>,
    colors: Res<theme::ChartColors>,
) {
    // Spawn camera
    commands.spawn(Camera2d);

    // Get window dimensions for responsive sizing
    let Ok((window_entity, window)) = window_query.single() else {
        eprintln!("Failed to get window");
        return;
    };

    // Initialize CursorIcon component on window (required in Bevy 0.17)
    commands
        .entity(window_entity)
        .insert(bevy::window::CursorIcon::default());

    // Initialize database connection
    let db = ChartDatabase::new(&args.db_path).expect("Failed to open database");

    // Resolve ticker symbol to ticker_id
    let ticker_id = resolve_ticker_id(&db, &args.ticker).expect("Failed to resolve ticker");

    // Parse date range
    let start_time = parse_date_to_timestamp(&args.start).expect("Failed to parse start date");
    let end_time = parse_date_to_timestamp(&args.end).expect("Failed to parse end date");

    println!(
        "Loading {} {} from {} to {} (timestamps: {} to {})",
        args.ticker, args.timeframe, args.start, args.end, start_time, end_time
    );

    // Load candles for specified date range
    let loaded_candles = db
        .load_candles(ticker_id, &args.timeframe, start_time, end_time)
        .expect("Failed to load candles");

    println!("Loaded {} candles", loaded_candles.len());

    // Convert Vec to slice for functions that need it
    let candles_slice: Vec<Candle> = loaded_candles.clone();

    // Calculate chart area dynamically based on window size
    // Chart occupies 70% of window width (left side)
    // Leave margins for axis labels (5% on each side vertically)
    let chart_width: f32 = window.width() * 0.85;
    let chart_height: f32 = window.height() * 0.9; // 90% of height to leave room for labels

    // Position chart in the left 70% of window
    // Window world space: -window.width/2 to +window.width/2
    // Chart center X = left_edge + (chart_width / 2)
    // let center_x = -window.width() / 2.0 + chart_width / 2.0;

    let total_area = Rect::from_center_size(
        // Vec2::new(center_x, 0.0),  // Centered in left 70% area
        Vec2::new(-chart_width * 0.325, 0.0), // Centered in left 70% area
        Vec2::new(chart_width * 0.95, chart_height), // Leave 5% horizontal margin for labels
    );

    // Phase 3: Calculate time-based visible range
    // Calculate timeframe interval in milliseconds
    let interval_ms: i64 = match args.timeframe.as_str() {
        "1m" => 60 * 1000,
        "3m" => 3 * 60 * 1000,
        "5m" => 5 * 60 * 1000,
        "15m" => 15 * 60 * 1000,
        "30m" => 30 * 60 * 1000,
        "1h" => 60 * 60 * 1000,
        "2h" => 2 * 60 * 60 * 1000,
        "4h" => 4 * 60 * 60 * 1000,
        "6h" => 6 * 60 * 60 * 1000,
        "8h" => 8 * 60 * 60 * 1000,
        "12h" => 12 * 60 * 60 * 1000,
        "1d" => 24 * 60 * 60 * 1000,
        "3d" => 3 * 24 * 60 * 60 * 1000,
        "1w" => 7 * 24 * 60 * 60 * 1000,
        "1M" => 30 * 24 * 60 * 60 * 1000,
        _ => 60 * 60 * 1000, // Default 1h
    };

    // Convert to BTreeMap for bounds fitting functions (do this early)
    let candles_btree: std::collections::BTreeMap<i64, Candle> = loaded_candles.iter().map(|c| (c.time, c.clone())).collect();

    // Calculate time-based visible range (show last 50 candles by default)
    let target_candle_count = 50.min(loaded_candles.len());
    let visible_duration = interval_ms * target_candle_count as i64;

    // Get time bounds from data
    let latest_time = candles_btree.keys().next_back().copied().unwrap_or(end_time);
    let earliest_time = candles_btree.keys().next().copied().unwrap_or(start_time);

    // Set visible range to show most recent data with some right spacing
    let right_spacing = right_spacing_duration(visible_duration);
    let visible_time_end = latest_time + right_spacing;
    let visible_time_start = (visible_time_end - visible_duration).max(earliest_time);

    // Initialize multi-pane layout: 70% Price + 30% Volume
    let mut panes = vec![
        Pane::new(
            PaneId::Price,
            PaneType::Price,
            0.7,             // 70% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            target_candle_count,
        ),
        Pane::new(
            PaneId::Volume,
            PaneType::Volume,
            0.3,             // 30% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            target_candle_count,
        ),
    ];

    // Calculate pane layouts
    calculate_pane_layouts(&mut panes, total_area, target_candle_count);

    // Calculate Moving Average indicators (before fitting bounds so we can include them)
    let indicators = vec![
        MovingAverage::new_sma(&candles_slice, 20, Color::srgb(1.0, 0.8, 0.0)), // Yellow SMA-20
        MovingAverage::new_sma(&candles_slice, 50, Color::srgb(0.0, 1.0, 1.0)), // Cyan SMA-50
        MovingAverage::new_sma(&candles_slice, 200, Color::srgb(1.0, 0.0, 1.0)), // Magenta SMA-200
    ];

    // Fit Y-axis bounds for each pane (including indicators for Price pane)
    // Phase 3: Use time-based bounds
    for pane in panes.iter_mut() {
        match pane.pane_type {
            PaneType::Price => {
                pane.space.fit_price_bounds_with_indicators(
                    &candles_btree,
                    &indicators,
                    visible_time_start,
                    visible_time_end,
                );
            }
            PaneType::Volume => {
                pane.space
                    .fit_volume_bounds(&candles_btree, visible_time_start, visible_time_end, CandlestickLODConfig::default().volume_y_axis_padding);
            }
            _ => {}
        }
    }

    // Create deprecated space for backward compatibility (not used in multi-pane)
    let space = ChartSpace::new(total_area, target_candle_count);

    let chart = Chart {
        ticker_id,
        timeframe: args.timeframe.clone(),
        candles: loaded_candles.into_iter().map(|c| (c.time, c)).collect(),
        candle_offset: 0,
        visible_time_start,
        visible_time_end,
        panes,
        total_area,
        space, // Deprecated
        indicators,
        needs_redraw: true,
        loading: false,
        load_status: ChartLoadStatus::Ready, // Frame-timing: start in ready state
    };

    // Initialize persistent crosshair entities (needs chart reference)
    init_crosshair(&mut commands, &chart, &colors);

    // Initialize timeframe manager
    let mut timeframe_mgr = TimeframeManager::new(ticker_id, args.timeframe.clone());

    // Populate available timeframes from database
    match db.get_available_timeframes(ticker_id) {
        Ok(timeframes) => {
            if !timeframes.is_empty() {
                println!("Available timeframes: {:?}", timeframes);
                timeframe_mgr.available_timeframes = timeframes;
            } else {
                println!(
                    "Warning: No timeframes found in database for ticker_id {}",
                    ticker_id
                );
            }
        }
        Err(e) => {
            eprintln!("Failed to get available timeframes: {}", e);
        }
    }

    commands.insert_resource(db);
    commands.insert_resource(chart);
    commands.insert_resource(timeframe_mgr);
    commands.insert_resource(InteractionState::default());
    commands.insert_resource(ChartGrid::default());
    commands.insert_resource(ChartAxes::default());
    commands.insert_resource(Crosshair::default());
    commands.insert_resource(VolumeToggleState::default());
    commands.insert_resource(ScreenshotCounter::default());

    // Aggregation system resources
    let agg_config = aggregation::AggregationConfig::default();
    let cache_size_bytes = agg_config.cache_size_mb * 1024 * 1024;
    commands.insert_resource(agg_config);
    commands.insert_resource(aggregation::AggregationCache::new(cache_size_bytes));
    commands.insert_resource(aggregation::AggregationState::default());
    commands.insert_resource(CandlestickLODConfig::default());

    println!("Setup complete! Press 'V' to toggle volume pane, 'F' to take screenshot.");
    println!("Press Ctrl+Left/Right to switch between timeframes.");
}

/// Initialize entity pools at startup
fn init_entity_pools(mut commands: Commands, config: Res<EntityPoolConfig>) {
    if !config.enabled {
        commands.insert_resource(EntityPools::new());
        return;
    }

    let mut pools = EntityPools::new();

    // Pre-allocate entities for each pool
    let pool_config = PoolSizeConfig::default();
    for i in 0..pool_config.initial_pool_size {
        // Wick pool
        let wick_entity = commands
            .spawn((
                Sprite {
                    color: Color::srgb(0.5, 0.5, 0.5),
                    custom_size: Some(Vec2::new(1.0, 1.0)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(-10000.0, -10000.0, 0.0)), // Off-screen
                Visibility::Hidden,
                CandlestickWick {
                    candle_timestamp: i64::MIN,
                },
                PooledEntity {
                    entity_type: PooledEntityType::CandlestickWick,
                    in_use: false,
                },
                PriceElement,
                PaneId::Price,
            ))
            .id();
        pools.wicks.add_entity(wick_entity);

        // Body pool
        let body_entity = commands
            .spawn((
                Sprite {
                    color: Color::srgb(0.0, 0.8, 0.2),
                    custom_size: Some(Vec2::new(1.0, 1.0)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(-10000.0, -10000.0, 1.0)),
                Visibility::Hidden,
                CandlestickBody {
                    candle_timestamp: i64::MIN,
                },
                PooledEntity {
                    entity_type: PooledEntityType::CandlestickBody,
                    in_use: false,
                },
                PriceElement,
                PaneId::Price,
            ))
            .id();
        pools.bodies.add_entity(body_entity);

        // OHLC pool
        let ohlc_entity = commands
            .spawn((
                Sprite {
                    color: Color::srgb(0.0, 0.8, 0.2),
                    custom_size: Some(Vec2::new(1.5, 1.0)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(-10000.0, -10000.0, 0.0)),
                Visibility::Hidden,
                CandlestickOHLCLine {
                    candle_timestamp: i64::MIN,
                },
                PooledEntity {
                    entity_type: PooledEntityType::CandlestickOHLC,
                    in_use: false,
                },
                PriceElement,
                PaneId::Price,
            ))
            .id();
        pools.ohlc_lines.add_entity(ohlc_entity);

        // Range pool
        let range_entity = commands
            .spawn((
                Sprite {
                    color: Color::srgb(0.0, 0.8, 0.2),
                    custom_size: Some(Vec2::new(0.5, 1.0)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(-10000.0, -10000.0, 0.0)),
                Visibility::Hidden,
                CandlestickRangeLine {
                    candle_timestamp: i64::MIN,
                },
                PooledEntity {
                    entity_type: PooledEntityType::CandlestickRange,
                    in_use: false,
                },
                PriceElement,
                PaneId::Price,
            ))
            .id();
        pools.range_lines.add_entity(range_entity);

        // Volume bar pool
        let volume_entity = commands
            .spawn((
                Sprite {
                    color: Color::srgba(0.0, 0.8, 0.2, 0.6),
                    custom_size: Some(Vec2::new(1.0, 1.0)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(-10000.0, -10000.0, 0.0)),
                Visibility::Hidden,
                VolumeBar {
                    candle_timestamp: i64::MIN,
                },
                PooledEntity {
                    entity_type: PooledEntityType::VolumeBar,
                    in_use: false,
                },
                VolumeElement,
                PaneId::Volume,
            ))
            .id();
        pools.volume_bars.add_entity(volume_entity);
    }

    println!(
        "Initialized entity pools: {} entities per type",
        pool_config.initial_pool_size
    );

    commands.insert_resource(pools);
}

// ============================================================================
// RENDERING CONTROL
// ============================================================================

/// Reset aggregation frame-local state at the start of each frame
/// This ensures the level_changed_this_frame flag is fresh for each frame
fn reset_aggregation_frame_state(mut agg_state: ResMut<aggregation::AggregationState>) {
    agg_state.begin_frame();
}

/// Reset the redraw flag after all rendering systems have completed
/// This prevents unnecessary entity despawn/spawn on every frame
fn reset_redraw_flag(mut chart: ResMut<Chart>) {
    if chart.needs_redraw {
        chart.needs_redraw = false;
    }
}

// ============================================================================
// SCREENSHOT
// ============================================================================

/// Resource to track screenshot counter for incremental filenames
#[derive(Resource, Default)]
struct ScreenshotCounter(u32);

/// System to capture screenshots when F is pressed
fn screenshot_on_keypress(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    focus: Res<FocusState>,
    mut counter: ResMut<ScreenshotCounter>,
) {
    // Only take screenshots when chart has focus
    if focus.current != FocusTarget::Chart {
        return;
    }

    if input.just_pressed(KeyCode::KeyF) {
        take_screenshot(&mut commands, None, &mut counter.0);
    }
}

/// BRP handler for taking screenshots remotely
fn handle_screenshot(
    In(params): In<Option<Value>>,
    mut commands: Commands,
    mut counter: Local<u32>,
) -> BrpResult {
    let path_param = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(|p| p.as_str())
        .map(String::from);

    // Generate the desired final path
    let final_path = path_param.unwrap_or_else(|| {
        let name = format!("screenshot-{:04}.png", *counter);
        *counter += 1;
        name
    });

    // Generate temp path (insert "-tmp" before extension)
    let temp_path = final_path.replace(".png", "-tmp.png");

    // Take screenshot to temp file
    take_screenshot(&mut commands, Some(temp_path.clone()), &mut 0);

    // Spawn a separate thread to wait for screenshot and crop it (don't join - let it run async)
    let temp_path_clone = temp_path.clone();
    let final_path_clone = final_path.clone();
    std::thread::spawn(move || {
        // Wait for temp file to exist (max 10 seconds, check every 50ms)
        let start = std::time::Instant::now();
        while !std::path::Path::new(&temp_path_clone).exists() {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if start.elapsed() > std::time::Duration::from_secs(10) {
                eprintln!("Screenshot timeout: temp file not created within 10 seconds");
                return;
            }
        }

        // Crop from temp to final path (left 70% = chart area only)
        if let Err(e) = crop_screenshot_to_chart(&temp_path_clone, &final_path_clone) {
            eprintln!("Failed to crop screenshot: {}", e);
            let _ = std::fs::remove_file(&temp_path_clone);
            return;
        }

        // Delete temp file
        if let Err(e) = std::fs::remove_file(&temp_path_clone) {
            eprintln!(
                "Warning: Failed to delete temp screenshot file {}: {}",
                temp_path_clone, e
            );
        }

        println!("Screenshot cropped successfully: {}", final_path_clone);
    });

    // Return immediately - the cropped file will be ready within 1-2 seconds
    // Claude's MCP client will need to wait a moment before reading
    Ok(serde_json::json!({
        "success": true,
        "path": final_path,
        "note": "Screenshot is being processed asynchronously. File will be ready within 1-2 seconds."
    }))
}

// ============================================================================
// FPS COUNTER
// ============================================================================

/// Component to mark the FPS counter text
#[derive(Component)]
struct FpsText;

/// Setup FPS counter UI in top-right corner
fn setup_fps_counter(mut commands: Commands) {
    commands.spawn((
        Text::new("FPS: --"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(0.0, 1.0, 0.0)), // Green text
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
        FpsText,
    ));
}

/// Update FPS counter text every frame
fn update_fps_counter(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                text.0 = format!("FPS: {:.0}", value);
            }
        }
    }
}

// ============================================================================
// TIMEFRAME LABEL
// ============================================================================

/// Component to mark the timeframe label text
#[derive(Component)]
struct TimeframeText;

/// Setup timeframe label UI in top-left corner
fn setup_timeframe_label(mut commands: Commands, args: Res<ChartArgs>) {
    commands.spawn((
        Text::new(format!("Timeframe: {}", args.timeframe)),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.8, 0.0)), // Yellow text
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        TimeframeText,
    ));
}

/// Update timeframe label when timeframe changes
fn update_timeframe_label(
    timeframe_mgr: Res<TimeframeManager>,
    chart: Res<Chart>,
    mut query: Query<&mut Text, With<TimeframeText>>,
) {
    if timeframe_mgr.is_changed() || chart.is_changed() {
        for mut text in &mut query {
            text.0 = format!(
                "Timeframe: {}\nCandles: {}",
                timeframe_mgr.current_timeframe,
                chart.visible_candle_count()
            );
        }
    }
}

// ============================================================================
// TIMEFRAME SWITCHING
// ============================================================================

/// Handle keyboard shortcuts for timeframe changes
fn handle_timeframe_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    timeframe_mgr: Res<TimeframeManager>,
    mut change_events: MessageWriter<TimeframeChangeRequest>,
) {
    let ctrl_pressed = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if !ctrl_pressed {
        return;
    }

    let current = timeframe_mgr.current_timeframe.clone();
    let ticker_id = timeframe_mgr.ticker_id;

    if keys.just_pressed(KeyCode::ArrowRight) {
        // Ctrl+Right: next larger timeframe
        if let Some(next) = timeframe_mgr.next_timeframe() {
            println!("Switching timeframe: {} -> {}", current, next);
            change_events.write(TimeframeChangeRequest {
                from: current,
                to: next,
                ticker_id,
            });
        } else {
            println!(
                "Already at largest available timeframe: {} (available: {:?})",
                current, timeframe_mgr.available_timeframes
            );
        }
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        // Ctrl+Left: previous smaller timeframe
        if let Some(prev) = timeframe_mgr.prev_timeframe() {
            println!("Switching timeframe: {} -> {}", current, prev);
            change_events.write(TimeframeChangeRequest {
                from: current,
                to: prev,
                ticker_id,
            });
        } else {
            println!(
                "Already at smallest available timeframe: {} (available: {:?})",
                current, timeframe_mgr.available_timeframes
            );
        }
    }
}

/// Handle timeframe change requests
fn handle_timeframe_change(
    mut chart: ResMut<Chart>,
    mut timeframe_mgr: ResMut<TimeframeManager>,
    config: Res<CandlestickLODConfig>,
    db: Res<ChartDatabase>,
    mut change_events: MessageReader<TimeframeChangeRequest>,
    args: Res<ChartArgs>,
) {
    for event in change_events.read() {
        println!(
            "Processing timeframe change: {} -> {}",
            event.from, event.to
        );

        // Load new data from database
        let start_time = parse_date_to_timestamp(&args.start).unwrap_or(0);
        let end_time = parse_date_to_timestamp(&args.end).unwrap_or(i64::MAX);

        match db.load_candles(event.ticker_id, &event.to, start_time, end_time) {
            Ok(loaded_candles) => {
                if loaded_candles.is_empty() {
                    eprintln!("No data available for timeframe: {}", event.to);
                    eprintln!("Try downloading data first with: cargo run --bin data-loader -- klines -t {} -i {} -s {} -e {}",
                        args.ticker, event.to, args.start, args.end);
                    continue;
                }

                println!(
                    "Loaded {} candles for timeframe: {}",
                    loaded_candles.len(),
                    event.to
                );

                // Keep a slice copy for functions that need &[Candle]
                let candles_slice = loaded_candles.clone();

                // Update chart with new data (convert to BTreeMap)
                chart.candles = loaded_candles.into_iter().map(|c| (c.time, c)).collect();
                chart.timeframe = event.to.clone();
                timeframe_mgr.current_timeframe = event.to.clone();

                // Phase 3: Reset visible range using time-based approach
                let interval_ms = chart.timeframe_interval_ms();
                let target_count = candles_slice.len().min(200); // Show up to 200 candles
                let visible_duration = interval_ms * target_count as i64;

                // Get time bounds from data
                let earliest = chart.candles.keys().next().copied().unwrap_or(0);
                let latest = chart.candles.keys().next_back().copied().unwrap_or(0);

                // Set visible range to show most recent data with some right spacing
                let right_spacing = right_spacing_duration(visible_duration);
                chart.visible_time_end = latest + right_spacing;
                chart.visible_time_start = (chart.visible_time_end - visible_duration).max(earliest);

                // Recalculate indicators with new data
                let new_indicators = vec![
                    MovingAverage::new_sma(&candles_slice, 20, Color::srgb(1.0, 0.8, 0.0)),
                    MovingAverage::new_sma(&candles_slice, 50, Color::srgb(0.0, 1.0, 1.0)),
                    MovingAverage::new_sma(&candles_slice, 200, Color::srgb(1.0, 0.0, 1.0)),
                ];

                // Update pane bounds
                let visible_time_start = chart.visible_time_start;
                let visible_time_end = chart.visible_time_end;
                let volume_padding = config.volume_y_axis_padding;

                chart.indicators = new_indicators;

                // Use a scoped block to split borrows cleanly
                {
                    let chart = chart.as_mut();
                    let candles = &chart.candles;
                    let indicators = &chart.indicators;
                    for pane in chart.panes.iter_mut() {
                        match pane.pane_type {
                            PaneType::Price => {
                                pane.space.fit_price_bounds_with_indicators(
                                    candles,
                                    indicators,
                                    visible_time_start,
                                    visible_time_end,
                                );
                            }
                            PaneType::Volume => {
                                pane.space.fit_volume_bounds(
                                    candles,
                                    visible_time_start,
                                    visible_time_end,
                                    volume_padding,
                                );
                            }
                            _ => {}
                        }
                        let visible_candle_count = candles.range(visible_time_start..=visible_time_end).count();
                        pane.space.recalculate_cache(
                            visible_candle_count,
                            crate::aggregation::AggregationLevel::None,
                            visible_candle_count
                        );
                    }
                }

                // Trigger redraw
                chart.needs_redraw = true;

                println!("Timeframe changed successfully to: {}", event.to);
            }
            Err(e) => {
                eprintln!("Failed to load timeframe {}: {}", event.to, e);
            }
        }
    }
}

/// Handle aggregation cache clearing on timeframe changes
fn handle_timeframe_change_aggregation(
    mut change_events: MessageReader<TimeframeChangeRequest>,
    mut cache: ResMut<aggregation::AggregationCache>,
    mut state: ResMut<aggregation::AggregationState>,
) {
    for event in change_events.read() {
        println!("Clearing aggregation cache for old timeframe: {}", event.from);
        cache.clear_timeframe(&event.from);

        // Reset aggregation state
        state.current_level = aggregation::AggregationLevel::None;
        state.level_change_cooldown = None;
    }
}
