//! Numeric constants extracted from magic numbers throughout the codebase.
//!
//! This module provides named constants for all hardcoded values, organized
//! by category. Using named constants improves code readability and makes
//! it easy to adjust values in one place.

// ============================================================================
// WINDOW & VIEWPORT
// ============================================================================

/// Default window width in pixels
pub const WINDOW_WIDTH: f32 = 1600.0;

/// Default window height in pixels
pub const WINDOW_HEIGHT: f32 = 900.0;

/// Chart area width (smaller than window to leave room for labels)
pub const CHART_AREA_WIDTH: f32 = 1400.0;

/// Chart area height (smaller than window to leave room for labels)
pub const CHART_AREA_HEIGHT: f32 = 780.0;

/// Chart area X offset from center (negative = left)
pub const CHART_AREA_OFFSET_X: f32 = -40.0;

/// Chart area Y offset from center (positive = up)
pub const CHART_AREA_OFFSET_Y: f32 = 10.0;

// ============================================================================
// PANE LAYOUT
// ============================================================================

/// Default height percentage for price pane (0.0-1.0)
pub const DEFAULT_PRICE_PANE_HEIGHT: f32 = 0.7;

/// Default height percentage for volume pane (0.0-1.0)
pub const DEFAULT_VOLUME_PANE_HEIGHT: f32 = 0.3;

/// Gap between panes in pixels (separator/resize area)
pub const PANE_SEPARATOR_GAP: f32 = 24.0;

/// Minimum pane height percentage (prevents panes from becoming too small)
pub const MIN_PANE_HEIGHT_PERCENT: f32 = 0.1;

/// Maximum pane height percentage (prevents one pane from taking all space)
pub const MAX_PANE_HEIGHT_PERCENT: f32 = 0.9;

/// Full height when only one pane is visible
pub const FULL_PANE_HEIGHT: f32 = 1.0;

/// Minimum change threshold for pane resize to register (prevents jitter)
pub const RESIZE_SENSITIVITY_THRESHOLD: f32 = 0.001;

// ============================================================================
// ZOOM & NAVIGATION
// ============================================================================

/// Zoom factor when scrolling up (zoom in) - less than 1.0 = fewer candles
pub const ZOOM_IN_FACTOR: f32 = 0.9;

/// Zoom factor when scrolling down (zoom out) - greater than 1.0 = more candles
pub const ZOOM_OUT_FACTOR: f32 = 1.1;

/// Minimum number of candles that can be visible
pub const MIN_VISIBLE_CANDLES: f32 = 10.0;

/// Maximum number of candles that can be visible
pub const MAX_VISIBLE_CANDLES: f32 = 10000.0;

/// Default number of visible candles on startup
pub const DEFAULT_VISIBLE_CANDLES: usize = 50;

/// Price axis padding percentage (extra space above/below price range)
pub const PRICE_PADDING_PERCENT: f32 = 0.05;

// ============================================================================
// DATA LOADING (Lazy Load)
// ============================================================================

/// Threshold in candles from edge to trigger lazy loading
pub const LAZY_LOAD_THRESHOLD: usize = 20;

/// Number of candles to load per lazy load request
pub const LAZY_LOAD_BATCH_SIZE: i64 = 100;

// ============================================================================
// RENDERING DIMENSIONS
// ============================================================================

/// Candlestick body width as a fraction of candle spacing (0.7 = 70%)
pub const CANDLE_BODY_WIDTH_RATIO: f32 = 0.7;

/// Volume bar width as a fraction of candle spacing (0.7 = 70%)
pub const VOLUME_BAR_WIDTH_RATIO: f32 = 0.7;

/// Minimum height for rendered elements in pixels
pub const MIN_ELEMENT_HEIGHT: f32 = 1.0;

/// Pane border thickness in pixels
pub const BORDER_THICKNESS: f32 = 2.0;

/// Grid line thickness in pixels
pub const GRID_LINE_THICKNESS: f32 = 1.0;

// ============================================================================
// CROSSHAIR PATTERN
// ============================================================================

/// Length of each dash segment in pixels
pub const CROSSHAIR_DASH_LENGTH: f32 = 8.0;

/// Length of gap between dashes in pixels
pub const CROSSHAIR_GAP_LENGTH: f32 = 4.0;

/// Crosshair line thickness in pixels
pub const CROSSHAIR_LINE_THICKNESS: f32 = 1.0;

// ============================================================================
// LABEL OFFSETS
// ============================================================================

/// X offset for Y-axis price labels (distance from chart right edge)
pub const PRICE_LABEL_OFFSET_X: f32 = 50.0;

/// Y offset for X-axis time labels (distance below chart bottom)
pub const TIME_LABEL_OFFSET_Y: f32 = 40.0;

/// X offset for OHLCV info box from left edge of price pane
pub const OHLCV_BOX_OFFSET_X: f32 = 100.0;

/// Y offset for OHLCV info box from top of price pane
pub const OHLCV_BOX_OFFSET_Y: f32 = 40.0;

// ============================================================================
// FONT SIZES
// ============================================================================

/// Font size for axis labels (price and time)
pub const AXIS_LABEL_FONT_SIZE: f32 = 16.0;

/// Font size for crosshair labels (price and time at cursor)
pub const CROSSHAIR_LABEL_FONT_SIZE: f32 = 14.0;

/// Font size for OHLCV info box
pub const OHLCV_BOX_FONT_SIZE: f32 = 16.0;

/// Font size for FPS counter
pub const FPS_COUNTER_FONT_SIZE: f32 = 20.0;

// ============================================================================
// GRID CONFIGURATION
// ============================================================================

/// Default number of horizontal grid lines (Y-axis ticks)
pub const DEFAULT_Y_TICK_COUNT: usize = 8;

/// Default number of vertical grid lines (X-axis ticks)
pub const DEFAULT_X_TICK_COUNT: usize = 10;

// ============================================================================
// Z-LAYER ORDERING (higher = more in front)
// ============================================================================

/// Z-layer for grid lines (background)
pub const Z_LAYER_GRID: f32 = -1.0;

/// Z-layer for volume bars
pub const Z_LAYER_VOLUME: f32 = 0.0;

/// Z-layer for pane borders
pub const Z_LAYER_PANE_BORDERS: f32 = 0.4;

/// Z-layer for resize grips
pub const Z_LAYER_RESIZE_GRIPS: f32 = 0.6;

/// Z-layer for axis labels
pub const Z_LAYER_AXIS_LABELS: f32 = 2.0;

/// Z-layer for crosshair lines
pub const Z_LAYER_CROSSHAIR_LINES: f32 = 3.0;

/// Z-layer for crosshair labels
pub const Z_LAYER_CROSSHAIR_LABELS: f32 = 4.0;

// ============================================================================
// INDICATOR DEFAULTS
// ============================================================================

/// Default SMA period - short term (typically displayed in yellow)
pub const DEFAULT_SMA_SHORT_PERIOD: usize = 20;

/// Default SMA period - medium term (typically displayed in cyan)
pub const DEFAULT_SMA_MEDIUM_PERIOD: usize = 50;

/// Default SMA period - long term (typically displayed in magenta)
pub const DEFAULT_SMA_LONG_PERIOD: usize = 200;

// ============================================================================
// RESIZE GRIP DIMENSIONS
// ============================================================================

/// Width of the resize grip indicator in pixels
pub const RESIZE_GRIP_WIDTH: f32 = 50.0;

/// Height of each line in the resize grip in pixels
pub const RESIZE_GRIP_LINE_HEIGHT: f32 = 2.0;

/// Spacing between lines in the resize grip in pixels
pub const RESIZE_GRIP_LINE_SPACING: f32 = 4.0;

// ============================================================================
// UI POSITIONING
// ============================================================================

/// FPS counter top margin in pixels
pub const FPS_COUNTER_TOP_MARGIN: f32 = 10.0;

/// FPS counter right margin in pixels
pub const FPS_COUNTER_RIGHT_MARGIN: f32 = 10.0;

// ============================================================================
// TRANSPARENCY VALUES
// ============================================================================

/// Alpha value for volume bars
pub const VOLUME_BAR_ALPHA: f32 = 0.6;

/// Alpha value for pane borders
pub const PANE_BORDER_ALPHA: f32 = 0.6;

/// Alpha value for crosshair lines
pub const CROSSHAIR_LINE_ALPHA: f32 = 0.6;

/// Alpha value for grid lines
pub const GRID_LINE_ALPHA: f32 = 0.3;

/// Alpha value for active resize grip
pub const RESIZE_GRIP_ACTIVE_ALPHA: f32 = 0.9;

/// Alpha value for inactive resize grip
pub const RESIZE_GRIP_INACTIVE_ALPHA: f32 = 0.7;
