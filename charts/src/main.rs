use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{PresentMode, WindowResolution};

use charts::config::{self, ChartDimensions, ChartTheme, InteractionConfig, ZLayerConfig};
use charts::coordinate::right_spacing_candles;
use charts::data::ChartDatabase;
use charts::interaction::{
    check_lazy_load, handle_pan, handle_resize, handle_zoom,
    toggle_sma_indicators, toggle_volume_pane, update_cursor_icon,
    update_cursor_position, update_interaction_mode, InteractionState,
};
use charts::panes::{Pane, PaneId, PaneManager, PaneType};
use charts::rendering::*;
use charts::rendering::{CandlestickInstancedPlugin, InstancingEnabled};
use charts::types::*;

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
    // Load dimensions config for window setup
    let dimensions = ChartDimensions::default();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Candlestick Chart".to_string(),
                resolution: WindowResolution::new(dimensions.window_width as u32, dimensions.window_height as u32),
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
        // Startup systems (setup must run first to insert config resources)
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_fps_counter.after(setup))
        // Input systems - handle user interaction first
        // Order: cursor position → state transitions → state handlers → keyboard
        .add_systems(
            Update,
            (
                // First: update cursor position
                update_cursor_position,
                // Second: determine state transitions
                update_interaction_mode.after(update_cursor_position),
                // Third: process current state handlers
                (handle_pan, handle_resize, handle_zoom).after(update_interaction_mode),
                // Fourth: update cursor icon based on state
                update_cursor_icon.after(update_interaction_mode),
                // Keyboard handlers (independent of mouse state)
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
    // Load configuration resources
    let theme = ChartTheme::default();
    let dimensions = ChartDimensions::default();
    let interaction_config = InteractionConfig::default();
    let z_layers = ZLayerConfig::default();

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
    let total_area = Rect::from_center_size(
        Vec2::new(dimensions.chart_area_offset_x, dimensions.chart_area_offset_y),
        Vec2::new(dimensions.chart_area_width, dimensions.chart_area_height),
    );

    let visible_candle_count = interaction_config.default_visible_candles.min(candles.len());
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

    // IndicatorState resource (using theme colors for indicators)
    let indicator_state = IndicatorState::new(vec![
        MovingAverage::new_sma(&candles, config::DEFAULT_SMA_SHORT_PERIOD, theme.sma_short),
        MovingAverage::new_sma(&candles, config::DEFAULT_SMA_MEDIUM_PERIOD, theme.sma_medium),
        MovingAverage::new_sma(&candles, config::DEFAULT_SMA_LONG_PERIOD, theme.sma_long),
    ]);

    // PaneManager resource (using config for default heights)
    let mut pane_manager = PaneManager::new_with_gap(
        vec![
            Pane::new(
                PaneId::Price,
                PaneType::Price,
                interaction_config.default_price_pane_height,
                Rect::default(), // Will be calculated by calculate_layouts
                visible_candle_count,
            ),
            Pane::new(
                PaneId::Volume,
                PaneType::Volume,
                interaction_config.default_volume_pane_height,
                Rect::default(), // Will be calculated by calculate_layouts
                visible_candle_count,
            ),
        ],
        dimensions.separator_gap,
    );

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

    // Insert configuration resources
    commands.insert_resource(theme.clone());
    commands.insert_resource(dimensions);
    commands.insert_resource(interaction_config);
    commands.insert_resource(z_layers);

    // Create ChartColors from theme for backward compatibility
    let chart_colors = ChartColors {
        bull_candle: theme.bull_candle,
        bear_candle: theme.bear_candle,
        wick: theme.wick,
    };

    // Insert other resources
    commands.insert_resource(db);
    commands.insert_resource(InteractionState::default());
    commands.insert_resource(ChartGrid::default());
    commands.insert_resource(ChartAxes::default());
    commands.insert_resource(Crosshair::default());
    commands.insert_resource(CrosshairState::default());
    commands.insert_resource(VolumeToggleState::default());
    commands.insert_resource(ScreenshotCounter::default());
    commands.insert_resource(chart_colors);
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
fn setup_fps_counter(mut commands: Commands, theme: Res<ChartTheme>, dimensions: Res<ChartDimensions>) {
    commands.spawn((
        Text::new("FPS: --"),
        TextFont {
            font_size: dimensions.fps_font_size,
            ..default()
        },
        TextColor(theme.fps_text),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(config::FPS_COUNTER_TOP_MARGIN),
            right: Val::Px(config::FPS_COUNTER_RIGHT_MARGIN),
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
