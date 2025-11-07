mod types;
mod rendering;
mod interaction;

use bevy::prelude::*;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, DiagnosticsStore};
use bevy::window::PresentMode;
use bevy::render::view::window::screenshot::ScreenshotManager;
use types::*;
use rendering::*;
use interaction::*;

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Candlestick Chart".to_string(),
                resolution: (1600.0, 900.0).into(),
                present_mode: PresentMode::AutoNoVsync, // Disable VSync for uncapped FPS
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_fps_counter)
        .add_systems(Update, (
            handle_mouse_input,
            update_crosshair,
        ).chain())  // Ensures crosshair updates immediately after mouse input
        .add_systems(Update, toggle_volume_pane)
        .add_systems(Update, toggle_sma_indicators)
        .add_systems(Update, check_lazy_load)
        .add_systems(Update, screenshot_on_keypress)
        .add_systems(Update, update_fps_counter)
        .add_systems(Update, (
            render_grid_and_axes,
            render_candlesticks,
            render_moving_averages,
            render_volume_bars,
            reset_redraw_flag,
        ).chain())  // Run rendering systems in sequence, then reset flag
        .run();
}

// ============================================================================
// SETUP
// ============================================================================

fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn(Camera2dBundle::default());

    // Initialize database connection
    let db_path = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb";
    let db = ChartDatabase::new(db_path)
        .expect("Failed to open database");

    // Load initial data
    let ticker_id = 1; // BTCUSDT
    let timeframe = "15m";

    // Get time range and load most recent candles
    let (min_time, max_time) = db.get_time_range(ticker_id, timeframe)
        .expect("Failed to get time range");

    println!("Database time range: {} to {}", min_time, max_time);

    let candles = db.load_candles(ticker_id, timeframe, min_time, max_time)
        .expect("Failed to load candles");

    println!("Loaded {} candles", candles.len());

    // Total chart area (leave room for axes labels)
    // Window is 1600x900, but we need margins for labels
    let total_area = Rect::from_center_size(
        Vec2::new(-40.0, 10.0),  // Offset left and up slightly
        Vec2::new(1400.0, 780.0), // Smaller than window to leave room for labels
    );

    let visible_candle_count = 50.min(candles.len());
    let spacing = right_spacing_candles(visible_candle_count);
    let visible_candle_start = (candles.len() + spacing).saturating_sub(visible_candle_count);

    // Initialize multi-pane layout: 70% Price + 30% Volume
    let mut panes = vec![
        Pane::new(
            PaneId::Price,
            PaneType::Price,
            0.7, // 70% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            visible_candle_count,
        ),
        Pane::new(
            PaneId::Volume,
            PaneType::Volume,
            0.3, // 30% of chart height
            Rect::default(), // Will be calculated by calculate_pane_layouts
            visible_candle_count,
        ),
    ];

    // Calculate pane layouts
    calculate_pane_layouts(&mut panes, total_area, visible_candle_count);

    // Calculate Moving Average indicators (before fitting bounds so we can include them)
    let indicators = vec![
        MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 0.8, 0.0)),  // Yellow SMA-20
        MovingAverage::new_sma(&candles, 50, Color::srgb(0.0, 1.0, 1.0)),  // Cyan SMA-50
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
                    visible_candle_count
                );
            }
            PaneType::Volume => {
                pane.space.fit_volume_bounds(&candles, visible_candle_start, visible_candle_count);
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
    input: Res<ButtonInput<KeyCode>>,
    mut screenshot_manager: ResMut<ScreenshotManager>,
    mut counter: ResMut<ScreenshotCounter>,
    primary_window: Query<Entity, With<Window>>,
) {
    if input.just_pressed(KeyCode::KeyF) {
        let filename = format!("screenshot-{:04}.png", counter.0);
        counter.0 += 1;

        println!("📸 Taking screenshot: {}", filename);

        if let Ok(window_entity) = primary_window.get_single() {
            screenshot_manager
                .save_screenshot_to_disk(window_entity, filename)
                .unwrap_or_else(|e| eprintln!("Failed to take screenshot: {}", e));
        }
    }
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
        TextBundle::from_section(
            "FPS: --",
            TextStyle {
                font_size: 20.0,
                color: Color::srgb(0.0, 1.0, 0.0), // Green text
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        }),
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
                text.sections[0].value = format!("FPS: {:.0}", value);
            }
        }
    }
}
