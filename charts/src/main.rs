mod interaction;
mod rendering;
mod types;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{PresentMode, WindowResolution};
use interaction::*;
use rendering::*;
use rendering::{CandlestickInstancedPlugin, InstancingEnabled};
use types::*;

// ============================================================================
// SYSTEM SETS
// ============================================================================

/// System sets for organizing chart systems into logical phases.
/// This provides explicit ordering to prevent race conditions and
/// ensures proper data flow between systems.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChartSystems {
    /// Input handling: mouse, keyboard, UI interactions
    Input,
    /// State updates: lazy loading, data changes
    StateUpdate,
    /// Rendering: grid, axes, candles, indicators
    Rendering,
    /// Cleanup: reset flags, prepare for next frame
    Cleanup,
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Candlestick Chart".to_string(),
                resolution: WindowResolution::new(1600, 900),
                present_mode: PresentMode::AutoNoVsync, // Disable VSync for uncapped FPS
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(CandlestickInstancedPlugin)
        // Configure system set ordering: Input -> StateUpdate -> Rendering -> Cleanup
        .configure_sets(
            Update,
            (
                ChartSystems::Input,
                ChartSystems::StateUpdate.after(ChartSystems::Input),
                ChartSystems::Rendering.after(ChartSystems::StateUpdate),
                ChartSystems::Cleanup.after(ChartSystems::Rendering),
            ),
        )
        // Startup systems
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_fps_counter)
        // Input systems - handle user interaction first
        .add_systems(
            Update,
            (
                handle_mouse_input,
                toggle_volume_pane,
                toggle_sma_indicators,
                screenshot_on_keypress,
            )
                .in_set(ChartSystems::Input),
        )
        // State update systems - update data based on input
        .add_systems(
            Update,
            (
                check_lazy_load,
                update_crosshair,
            )
                .chain()  // Crosshair needs updated data from lazy load
                .in_set(ChartSystems::StateUpdate),
        )
        // Rendering systems - draw based on current state
        // Split grid rendering into focused systems for better parallelization
        .add_systems(
            Update,
            (
                // Grid systems can run in parallel (no dependencies between them)
                render_grid_lines,
                render_pane_borders,
                render_resize_grips,
                render_axis_labels,
                // Then render data elements
                render_moving_averages,
                render_volume_bars,
            )
                .in_set(ChartSystems::Rendering),
        )
        // Cleanup systems - reset flags after rendering
        .add_systems(Update, reset_redraw_flag.in_set(ChartSystems::Cleanup))
        // Utility systems (run independently)
        .add_systems(Update, update_fps_counter)
        .run();
}

// ============================================================================
// SETUP
// ============================================================================

fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn(Camera2d);

    // Initialize database connection
    let db_path = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb";
    let db = ChartDatabase::new(db_path).expect("Failed to open database");

    // Load initial data
    let ticker_id = 4; // ticker with 1m data
    let timeframe = "1m";

    // Get time range and load most recent candles
    let (min_time, max_time) = db
        .get_time_range(ticker_id, timeframe)
        .expect("Failed to get time range");

    println!("Database time range: {} to {}", min_time, max_time);

    let candles = db
        .load_candles(ticker_id, timeframe, min_time, max_time)
        .expect("Failed to load candles");

    println!("Loaded {} candles", candles.len());

    // Total chart area (leave room for axes labels)
    // Window is 1600x900, but we need margins for labels
    let total_area = Rect::from_center_size(
        Vec2::new(-40.0, 10.0),   // Offset left and up slightly
        Vec2::new(1400.0, 780.0), // Smaller than window to leave room for labels
    );

    let visible_candle_count = 50.min(candles.len());
    let spacing = right_spacing_candles(visible_candle_count);
    let visible_candle_start = (candles.len() + spacing).saturating_sub(visible_candle_count);

    // === NEW: Create separated resources ===

    // CandleData resource
    let candle_data = CandleData {
        candles: candles.clone(),
        candle_offset: 0,
    };

    // ChartMetadata resource
    let chart_metadata = ChartMetadata::new(ticker_id, timeframe);

    // ViewportState resource
    let viewport_state = ViewportState {
        visible_candle_start,
        visible_candle_count,
        total_area,
        needs_redraw: true,
        loading: false,
    };

    // IndicatorState resource
    let indicator_state = IndicatorState::new(vec![
        MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 0.8, 0.0)), // Yellow SMA-20
        MovingAverage::new_sma(&candles, 50, Color::srgb(0.0, 1.0, 1.0)), // Cyan SMA-50
        MovingAverage::new_sma(&candles, 200, Color::srgb(1.0, 0.0, 1.0)), // Magenta SMA-200
    ]);

    // PaneManager resource
    let mut pane_manager = PaneManager::new(vec![
        Pane::new(
            PaneId::Price,
            PaneType::Price,
            0.7,             // 70% of chart height
            Rect::default(), // Will be calculated by calculate_layouts
            visible_candle_count,
        ),
        Pane::new(
            PaneId::Volume,
            PaneType::Volume,
            0.3,             // 30% of chart height
            Rect::default(), // Will be calculated by calculate_layouts
            visible_candle_count,
        ),
    ]);

    // Calculate pane layouts using PaneManager
    pane_manager.calculate_layouts(total_area, visible_candle_count);

    // Fit Y-axis bounds for each pane
    pane_manager.update_pane_bounds(
        &candle_data.candles,
        &indicator_state.indicators,
        visible_candle_start,
        visible_candle_count,
    );

    // Initialize persistent crosshair entities
    init_crosshair(&mut commands, &pane_manager);

    // Insert separated resources
    commands.insert_resource(candle_data);
    commands.insert_resource(chart_metadata);
    commands.insert_resource(viewport_state);
    commands.insert_resource(indicator_state);
    commands.insert_resource(pane_manager);

    // Insert other resources
    commands.insert_resource(db);
    commands.insert_resource(InteractionState::default());
    commands.insert_resource(ChartGrid::default());
    commands.insert_resource(ChartAxes::default());
    commands.insert_resource(Crosshair::default());
    commands.insert_resource(CrosshairState::default());
    commands.insert_resource(VolumeToggleState::default());
    commands.insert_resource(ScreenshotCounter::default());
    commands.insert_resource(ChartColors::default());
    commands.insert_resource(InstancingEnabled::default());

    println!("Setup complete! Press 'V' to toggle volume pane, 'F' to take screenshot.");
}

// ============================================================================
// RENDERING CONTROL
// ============================================================================

/// Reset the redraw flag after all rendering systems have completed
/// This prevents unnecessary entity despawn/spawn on every frame
fn reset_redraw_flag(mut viewport_state: ResMut<ViewportState>) {
    if viewport_state.needs_redraw {
        viewport_state.needs_redraw = false;
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
    mut commands: Commands,
    mut counter: ResMut<ScreenshotCounter>,
) {
    if input.just_pressed(KeyCode::KeyF) {
        let filename = format!("screenshot-{:04}.png", counter.0);
        counter.0 += 1;

        println!("Taking screenshot: {}", filename);

        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(filename));
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
                **text = format!("FPS: {:.0}", value);
            }
        }
    }
}
