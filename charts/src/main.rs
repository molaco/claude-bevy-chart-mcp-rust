mod focus;
mod interaction;
mod rendering;
mod screenshot;
mod types;
mod ui_layout;

use anyhow::{Context, Result};
use bevy::camera::Camera2d;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy::remote::{BrpError, BrpResult, RemotePlugin};
use bevy::window::PresentMode;
use chat_ui::prelude::*;
use chrono::NaiveDate;
use clap::Parser;
use focus::*;
use interaction::*;
use rendering::*;
use screenshot::{take_screenshot, crop_screenshot_to_chart};
use serde_json::Value;
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
    #[arg(long, default_value = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb")]
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
    let count = db.check_data_exists(ticker_id, timeframe, start_time, end_time)
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
            "Chart: {} candles visible (indices {}-{}), price range: ${:.2}-${:.2}, {} indicators active, volume pane: {}",
            chart.visible_candle_count,
            chart.visible_candle_start,
            chart.visible_candle_start + chart.visible_candle_count,
            chart.panes.get(0).map_or(0.0, |p| p.space.visible_price_min),
            chart.panes.get(0).map_or(0.0, |p| p.space.visible_price_max),
            chart.indicators.iter().filter(|i| i.visible).count(),
            if chart.panes.len() > 1 { "visible" } else { "hidden" }
        );
        chat_state.chart_context = Some(context);
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
            println!("Loading chart for {} {} from {} to {}",
                args.ticker, args.timeframe, args.start, args.end);
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
        .insert_resource(args) // Inject CLI args as a resource
        .init_resource::<FocusState>()
        .add_message::<TimeframeChangeRequest>()
        .add_systems(Startup, ui_layout::setup_split_layout)
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_fps_counter)
        .add_systems(Startup, setup_timeframe_label)
        .add_systems(PostStartup, ui_layout::reparent_chat_to_container)
        .add_systems(Update, update_chart_context)
        .add_systems(Update, (handle_focus_tab, handle_focus_click, update_focus_indicators, manage_text_input_focus).chain())
        .add_systems(Update, (handle_mouse_input, update_crosshair).chain()) // Ensures crosshair updates immediately after mouse input
        .add_systems(Update, toggle_volume_pane)
        .add_systems(Update, toggle_sma_indicators)
        .add_systems(Update, check_lazy_load)
        .add_systems(Update, screenshot_on_keypress)
        .add_systems(Update, update_fps_counter)
        .add_systems(Update, update_timeframe_label)
        .add_systems(Update, handle_timeframe_keyboard)
        .add_systems(Update, handle_timeframe_change)
        .add_systems(
            Update,
            (
                render_grid_and_axes,
                render_candlesticks,
                render_moving_averages,
                render_volume_bars,
                reset_redraw_flag,
            )
                .chain(),
        ) // Run rendering systems in sequence, then reset flag
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
    let count = validate_data_exists(&db, ticker_id, &args.timeframe, start_time, end_time, &args.ticker)?;

    println!("Found {} klines in database", count);

    Ok(())
}

// ============================================================================
// SETUP
// ============================================================================

fn setup(
    mut commands: Commands,
    window_query: Query<(Entity, &Window), With<Window>>,
    args: Res<ChartArgs>,
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
    let candles = db
        .load_candles(ticker_id, &args.timeframe, start_time, end_time)
        .expect("Failed to load candles");

    println!("Loaded {} candles", candles.len());

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

    let visible_candle_count = 50.min(candles.len());
    let spacing = right_spacing_candles(visible_candle_count);
    let visible_candle_start = (candles.len() + spacing).saturating_sub(visible_candle_count);

    // Initialize multi-pane layout: 70% Price + 30% Volume
    let mut panes = vec![
        Pane::new(
            PaneId::Price,
            PaneType::Price,
            0.7,             // 70% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            visible_candle_count,
        ),
        Pane::new(
            PaneId::Volume,
            PaneType::Volume,
            0.3,             // 30% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            visible_candle_count,
        ),
    ];

    // Calculate pane layouts
    calculate_pane_layouts(&mut panes, total_area, visible_candle_count);

    // Calculate Moving Average indicators (before fitting bounds so we can include them)
    let indicators = vec![
        MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 0.8, 0.0)), // Yellow SMA-20
        MovingAverage::new_sma(&candles, 50, Color::srgb(0.0, 1.0, 1.0)), // Cyan SMA-50
        MovingAverage::new_sma(&candles, 200, Color::srgb(1.0, 0.0, 1.0)), // Magenta SMA-200
    ];

    // Fit Y-axis bounds for each pane (including indicators for Price pane)
    for pane in panes.iter_mut() {
        match pane.pane_type {
            PaneType::Price => {
                pane.space.fit_price_bounds_with_indicators(
                    &candles,
                    &indicators,
                    visible_candle_start,
                    visible_candle_count,
                );
            }
            PaneType::Volume => {
                pane.space
                    .fit_volume_bounds(&candles, visible_candle_start, visible_candle_count);
            }
            _ => {}
        }
    }

    // Create deprecated space for backward compatibility (not used in multi-pane)
    let space = ChartSpace::new(total_area, visible_candle_count);

    let chart = Chart {
        ticker_id,
        timeframe: args.timeframe.clone(),
        candles,
        candle_offset: 0,
        visible_candle_start,
        visible_candle_count,
        panes,
        total_area,
        space, // Deprecated
        indicators,
        needs_redraw: true,
        loading: false,
    };

    // Initialize persistent crosshair entities (needs chart reference)
    init_crosshair(&mut commands, &chart);

    // Initialize timeframe manager
    let mut timeframe_mgr = TimeframeManager::new(ticker_id, args.timeframe.clone());

    // Populate available timeframes from database
    match db.get_available_timeframes(ticker_id) {
        Ok(timeframes) => {
            if !timeframes.is_empty() {
                println!("Available timeframes: {:?}", timeframes);
                timeframe_mgr.available_timeframes = timeframes;
            } else {
                println!("Warning: No timeframes found in database for ticker_id {}", ticker_id);
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

    println!("Setup complete! Press 'V' to toggle volume pane, 'F' to take screenshot.");
    println!("Press Ctrl+Left/Right to switch between timeframes.");
}

// ============================================================================
// RENDERING CONTROL
// ============================================================================

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
            eprintln!("Warning: Failed to delete temp screenshot file {}: {}", temp_path_clone, e);
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
    mut query: Query<&mut Text, With<TimeframeText>>,
) {
    if timeframe_mgr.is_changed() {
        for mut text in &mut query {
            text.0 = format!("Timeframe: {}", timeframe_mgr.current_timeframe);
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
            println!("Already at largest available timeframe: {} (available: {:?})",
                current, timeframe_mgr.available_timeframes);
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
            println!("Already at smallest available timeframe: {} (available: {:?})",
                current, timeframe_mgr.available_timeframes);
        }
    }
}

/// Handle timeframe change requests
fn handle_timeframe_change(
    mut chart: ResMut<Chart>,
    mut timeframe_mgr: ResMut<TimeframeManager>,
    db: Res<ChartDatabase>,
    mut change_events: MessageReader<TimeframeChangeRequest>,
    args: Res<ChartArgs>,
) {
    for event in change_events.read() {
        println!("Processing timeframe change: {} -> {}", event.from, event.to);

        // Load new data from database
        let start_time = parse_date_to_timestamp(&args.start).unwrap_or(0);
        let end_time = parse_date_to_timestamp(&args.end).unwrap_or(i64::MAX);

        match db.load_candles(event.ticker_id, &event.to, start_time, end_time) {
            Ok(candles) => {
                if candles.is_empty() {
                    eprintln!("No data available for timeframe: {}", event.to);
                    eprintln!("Try downloading data first with: cargo run --bin data-loader -- klines -t {} -i {} -s {} -e {}",
                        args.ticker, event.to, args.start, args.end);
                    continue;
                }

                println!("Loaded {} candles for timeframe: {}", candles.len(), event.to);

                // Update chart with new data
                chart.candles = candles.clone();
                chart.timeframe = event.to.clone();
                timeframe_mgr.current_timeframe = event.to.clone();

                // Reset visible range to show all candles
                chart.visible_candle_start = 0;
                chart.visible_candle_count = candles.len().min(200); // Show up to 200 candles

                // Recalculate indicators with new data
                let new_indicators = vec![
                    MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 0.8, 0.0)),
                    MovingAverage::new_sma(&candles, 50, Color::srgb(0.0, 1.0, 1.0)),
                    MovingAverage::new_sma(&candles, 200, Color::srgb(1.0, 0.0, 1.0)),
                ];

                // Update pane bounds (clone data to avoid borrow checker issues)
                let visible_start = chart.visible_candle_start;
                let visible_count = chart.visible_candle_count;
                let candles_clone = candles.clone();
                let indicators_clone = new_indicators.clone();

                chart.indicators = new_indicators;

                for pane in &mut chart.panes {
                    match pane.pane_type {
                        PaneType::Price => {
                            pane.space.fit_price_bounds_with_indicators(
                                &candles_clone,
                                &indicators_clone,
                                visible_start,
                                visible_count,
                            );
                        }
                        PaneType::Volume => {
                            pane.space.fit_volume_bounds(
                                &candles_clone,
                                visible_start,
                                visible_count,
                            );
                        }
                        _ => {}
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
