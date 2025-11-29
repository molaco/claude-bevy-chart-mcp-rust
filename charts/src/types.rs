use bevy::prelude::*;

// Re-export domain types for backward compatibility
pub use crate::domain::{Candle, Timeframe};

// Re-export indicator types for backward compatibility
pub use crate::indicators::{IndicatorState, MovingAverage};

// Re-export coordinate types for backward compatibility
pub use crate::coordinate::{right_spacing_candles, ChartSpace, ViewportState};

// Re-export panes types for backward compatibility
pub use crate::panes::{CrosshairEntities, Pane, PaneId, PaneManager, PaneType};

// Re-export data types for backward compatibility
pub use crate::data::{CandleData, ChartDatabase};

// Re-export interaction types for backward compatibility
pub use crate::interaction::{InteractionMode, InteractionState};

// ============================================================================
// SEPARATED RESOURCES (Decomposed from Chart god object)
// ============================================================================

/// Chart metadata resource - holds ticker and timeframe info
#[derive(Resource)]
pub struct ChartMetadata {
    pub ticker_id: i32,
    /// String representation for database queries
    timeframe_str: String,
    /// Type-safe timeframe (may be None for unsupported timeframes)
    timeframe_enum: Option<Timeframe>,
}

impl Default for ChartMetadata {
    fn default() -> Self {
        Self {
            ticker_id: 0,
            timeframe_str: "1m".to_string(),
            timeframe_enum: Some(Timeframe::M1),
        }
    }
}

impl ChartMetadata {
    /// Create from string timeframe (for backward compatibility)
    pub fn new(ticker_id: i32, timeframe: &str) -> Self {
        Self {
            ticker_id,
            timeframe_str: timeframe.to_string(),
            timeframe_enum: timeframe.parse().ok(),
        }
    }

    /// Create from type-safe Timeframe enum
    pub fn with_timeframe(ticker_id: i32, timeframe: Timeframe) -> Self {
        Self {
            ticker_id,
            timeframe_str: timeframe.as_str().to_string(),
            timeframe_enum: Some(timeframe),
        }
    }

    /// Get timeframe as string (for database queries)
    pub fn timeframe(&self) -> &str {
        &self.timeframe_str
    }

    /// Get type-safe timeframe (None if unsupported)
    pub fn timeframe_typed(&self) -> Option<Timeframe> {
        self.timeframe_enum
    }

    /// Calculate interval in milliseconds based on timeframe
    pub fn interval_ms(&self) -> i64 {
        // Use typed timeframe if available, otherwise fall back to string matching
        if let Some(tf) = self.timeframe_enum {
            tf.to_ms()
        } else {
            // Legacy fallback for unsupported timeframes
            match self.timeframe_str.as_str() {
                "1m" => 60 * 1000,
                "15m" => 15 * 60 * 1000,
                "1h" => 60 * 60 * 1000,
                "4h" => 4 * 60 * 60 * 1000,
                "1d" => 24 * 60 * 60 * 1000,
                _ => 60 * 60 * 1000,
            }
        }
    }
}


/// Volume pane toggle state
#[derive(Resource)]
pub struct VolumeToggleState {
    pub visible: bool,
}

impl Default for VolumeToggleState {
    fn default() -> Self {
        Self { visible: true }  // Volume pane visible by default
    }
}

/// Grid configuration
#[derive(Resource)]
pub struct ChartGrid {
    pub show_grid: bool,
    pub grid_color: Color,
    pub y_tick_count: usize,      // Number of horizontal grid lines
    pub x_tick_count: usize,      // Number of vertical grid lines
}

impl Default for ChartGrid {
    fn default() -> Self {
        let theme = crate::config::ChartTheme::default();
        Self {
            show_grid: true,
            grid_color: theme.grid_line,
            y_tick_count: crate::config::DEFAULT_Y_TICK_COUNT,
            x_tick_count: crate::config::DEFAULT_X_TICK_COUNT,
        }
    }
}

/// Axes configuration
#[derive(Resource)]
pub struct ChartAxes {
    pub show_x_labels: bool,
    pub show_y_labels: bool,
    pub label_color: Color,
    pub label_size: f32,
}

impl Default for ChartAxes {
    fn default() -> Self {
        let theme = crate::config::ChartTheme::default();
        Self {
            show_x_labels: true,
            show_y_labels: true,
            label_color: theme.axis_label,
            label_size: crate::config::AXIS_LABEL_FONT_SIZE,
        }
    }
}

/// Crosshair configuration
#[derive(Resource)]
pub struct Crosshair {
    pub enabled: bool,
    pub line_color: Color,
    pub show_ohlcv_box: bool,
    pub show_price_label: bool,
    pub show_time_label: bool,
}

impl Default for Crosshair {
    fn default() -> Self {
        let theme = crate::config::ChartTheme::default();
        Self {
            enabled: true,
            line_color: theme.crosshair_line,
            show_ohlcv_box: true,
            show_price_label: true,
            show_time_label: true,
        }
    }
}

/// Crosshair state resource - tracks crosshair position and visibility
#[derive(Resource, Default)]
pub struct CrosshairState {
    pub visible: bool,
    pub mouse_pos: Vec2,
    pub hovered_candle: Option<usize>,
    pub last_update_pos: Vec2,
}

/// Chart color theme
#[derive(Resource, Clone)]
pub struct ChartColors {
    pub bull_candle: Color,
    pub bear_candle: Color,
    pub wick: Color,
}

impl Default for ChartColors {
    fn default() -> Self {
        // Use theme colors as the source of truth
        let theme = crate::config::ChartTheme::default();
        Self {
            bull_candle: theme.bull_candle,
            bear_candle: theme.bear_candle,
            wick: theme.wick,
        }
    }
}

// ============================================================================
// COMPONENTS
// ============================================================================

/// Component to identify candlestick parts
#[derive(Component)]
pub struct CandlestickWick {
    pub candle_index: usize,
}

#[derive(Component)]
pub struct CandlestickBody {
    pub candle_index: usize,
}

/// Component to identify volume bars
#[derive(Component)]
pub struct VolumeBar {
    pub candle_index: usize,
}

/// Marker component for chart elements
#[derive(Component)]
pub struct ChartElement;

/// Marker component for price pane elements (candlestick wicks and bodies)
#[derive(Component)]
pub struct PriceElement;

/// Marker component for volume pane elements (volume bars)
#[derive(Component)]
pub struct VolumeElement;

/// Marker component for indicator elements (MA lines, etc.)
#[derive(Component)]
pub struct IndicatorElement;

/// Marker component for grid elements (lines and labels) - LEGACY, prefer specific markers
#[derive(Component)]
pub struct GridElement;

/// Marker component for grid lines only (horizontal and vertical)
#[derive(Component)]
pub struct GridLineElement;

/// Marker component for pane border elements
#[derive(Component)]
pub struct PaneBorderElement;

/// Marker component for resize grip elements (between panes)
#[derive(Component)]
pub struct ResizeGripElement;

/// Marker component for axis label elements (X and Y axis)
#[derive(Component)]
pub struct AxisLabelElement;

/// Marker component for crosshair elements
#[derive(Component)]
pub struct CrosshairElement;
