mod interaction;
mod rendering;
mod screenshot;
mod types;
mod ui_layout;

use bevy::camera::Camera2d;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy::remote::{BrpResult, RemotePlugin};
use bevy::window::PresentMode;
use chat_ui::prelude::*;
use interaction::*;
use rendering::*;
use screenshot::take_screenshot;
use serde_json::Value;
use types::*;
use ui_layout::ChartViewport;

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
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Candlestick Chart".to_string(),
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
        .add_systems(Startup, ui_layout::setup_split_layout)
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_fps_counter)
        .add_systems(PostStartup, ui_layout::reparent_chat_to_container)
        .add_systems(Update, update_chart_context)
        .add_systems(Update, (handle_mouse_input, update_crosshair).chain()) // Ensures crosshair updates immediately after mouse input
        .add_systems(Update, toggle_volume_pane)
        .add_systems(Update, toggle_sma_indicators)
        .add_systems(Update, check_lazy_load)
        .add_systems(Update, screenshot_on_keypress)
        .add_systems(Update, update_fps_counter)
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

// ============================================================================
// SETUP
// ============================================================================

fn setup(mut commands: Commands, window_query: Query<(Entity, &Window), With<Window>>) {
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
    let db_path = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb";
    let db = ChartDatabase::new(db_path).expect("Failed to open database");

    // Load initial data
    let ticker_id = 1; // BTCUSDT
    let timeframe = "15m";

    // Get time range and load most recent candles
    let (min_time, max_time) = db
        .get_time_range(ticker_id, timeframe)
        .expect("Failed to get time range");

    println!("Database time range: {} to {}", min_time, max_time);

    let candles = db
        .load_candles(ticker_id, timeframe, min_time, max_time)
        .expect("Failed to load candles");

    println!("Loaded {} candles", candles.len());

    // Calculate chart area dynamically based on window size
    // Chart occupies 70% of window width (left side)
    // Leave margins for axis labels (5% on each side vertically)
    //TODO: fix this
    let chart_width: f32 = window.width() * 0.85;
    let chart_height: f32 = window.height() * 0.9; // 90% of height to leave room for labels

    // Position chart in the left 70% of window
    // Window world space: -window.width/2 to +window.width/2
    // Chart center X = left_edge + (chart_width / 2)
    // let center_x = -window.width() / 2.0 + chart_width / 2.0;

    //TODO: change this bullshit to percentages
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
        timeframe: timeframe.to_string(),
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

    commands.insert_resource(db);
    commands.insert_resource(chart);
    commands.insert_resource(InteractionState::default());
    commands.insert_resource(ChartGrid::default());
    commands.insert_resource(ChartAxes::default());
    commands.insert_resource(Crosshair::default());
    commands.insert_resource(VolumeToggleState::default());
    commands.insert_resource(ScreenshotCounter::default());

    println!("Setup complete! Press 'V' to toggle volume pane, 'F' to take screenshot.");
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
    mut counter: ResMut<ScreenshotCounter>,
) {
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

    let path = take_screenshot(&mut commands, path_param, &mut counter);

    Ok(serde_json::json!({
        "success": true,
        "path": path
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
