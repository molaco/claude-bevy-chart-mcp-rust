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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // WINDOW & VIEWPORT CONSTRAINT TESTS
    // ========================================================================

    #[test]
    fn test_chart_area_fits_window() {
        assert!(
            CHART_AREA_WIDTH < WINDOW_WIDTH,
            "Chart area width ({}) must be less than window width ({})",
            CHART_AREA_WIDTH, WINDOW_WIDTH
        );
        assert!(
            CHART_AREA_HEIGHT < WINDOW_HEIGHT,
            "Chart area height ({}) must be less than window height ({})",
            CHART_AREA_HEIGHT, WINDOW_HEIGHT
        );
    }

    #[test]
    fn test_window_dimensions_positive() {
        assert!(WINDOW_WIDTH > 0.0, "Window width must be positive");
        assert!(WINDOW_HEIGHT > 0.0, "Window height must be positive");
        assert!(CHART_AREA_WIDTH > 0.0, "Chart area width must be positive");
        assert!(CHART_AREA_HEIGHT > 0.0, "Chart area height must be positive");
    }

    // ========================================================================
    // PANE LAYOUT CONSTRAINT TESTS
    // ========================================================================

    #[test]
    fn test_pane_height_constraints_valid() {
        assert!(
            MIN_PANE_HEIGHT_PERCENT > 0.0,
            "Min pane height must be positive"
        );
        assert!(
            MAX_PANE_HEIGHT_PERCENT < 1.0,
            "Max pane height must be less than 1.0 to allow other panes"
        );
        assert!(
            MIN_PANE_HEIGHT_PERCENT < MAX_PANE_HEIGHT_PERCENT,
            "Min pane height must be less than max"
        );
        assert!(
            MIN_PANE_HEIGHT_PERCENT * 2.0 <= 1.0,
            "Two panes at minimum height must fit (2 * {} <= 1.0)",
            MIN_PANE_HEIGHT_PERCENT
        );
    }

    #[test]
    fn test_default_pane_heights_sum_to_one() {
        let sum = DEFAULT_PRICE_PANE_HEIGHT + DEFAULT_VOLUME_PANE_HEIGHT;
        assert!(
            (sum - 1.0).abs() < 0.001,
            "Default pane heights should sum to 1.0, got {}",
            sum
        );
    }

    #[test]
    fn test_default_pane_heights_within_constraints() {
        assert!(
            DEFAULT_PRICE_PANE_HEIGHT >= MIN_PANE_HEIGHT_PERCENT,
            "Default price pane height below minimum"
        );
        assert!(
            DEFAULT_PRICE_PANE_HEIGHT <= MAX_PANE_HEIGHT_PERCENT,
            "Default price pane height above maximum"
        );
        assert!(
            DEFAULT_VOLUME_PANE_HEIGHT >= MIN_PANE_HEIGHT_PERCENT,
            "Default volume pane height below minimum"
        );
        assert!(
            DEFAULT_VOLUME_PANE_HEIGHT <= MAX_PANE_HEIGHT_PERCENT,
            "Default volume pane height above maximum"
        );
    }

    #[test]
    fn test_separator_gap_positive() {
        assert!(PANE_SEPARATOR_GAP > 0.0, "Separator gap must be positive");
    }

    #[test]
    fn test_resize_sensitivity_positive() {
        assert!(
            RESIZE_SENSITIVITY_THRESHOLD > 0.0,
            "Resize sensitivity must be positive"
        );
        assert!(
            RESIZE_SENSITIVITY_THRESHOLD < 0.1,
            "Resize sensitivity should be small for smooth resizing"
        );
    }

    // ========================================================================
    // ZOOM CONSTRAINT TESTS
    // ========================================================================

    #[test]
    fn test_zoom_factors_valid() {
        assert!(
            ZOOM_IN_FACTOR > 0.0 && ZOOM_IN_FACTOR < 1.0,
            "Zoom in factor ({}) must be in (0, 1)",
            ZOOM_IN_FACTOR
        );
        assert!(
            ZOOM_OUT_FACTOR > 1.0,
            "Zoom out factor ({}) must be > 1.0",
            ZOOM_OUT_FACTOR
        );
    }

    #[test]
    fn test_zoom_factors_inversely_related() {
        // Zoom in then out should approximately return to original
        let result = ZOOM_IN_FACTOR * ZOOM_OUT_FACTOR;
        assert!(
            (result - 1.0).abs() < 0.05,
            "Zoom factors should be approximately inverse: {} * {} = {}",
            ZOOM_IN_FACTOR, ZOOM_OUT_FACTOR, result
        );
    }

    #[test]
    fn test_visible_candle_constraints() {
        assert!(
            MIN_VISIBLE_CANDLES > 0.0,
            "Min visible candles must be positive"
        );
        assert!(
            MAX_VISIBLE_CANDLES > MIN_VISIBLE_CANDLES,
            "Max visible candles must exceed minimum"
        );
        assert!(
            (DEFAULT_VISIBLE_CANDLES as f32) >= MIN_VISIBLE_CANDLES,
            "Default visible candles below minimum"
        );
        assert!(
            (DEFAULT_VISIBLE_CANDLES as f32) <= MAX_VISIBLE_CANDLES,
            "Default visible candles above maximum"
        );
    }

    // ========================================================================
    // Z-LAYER ORDERING TESTS
    // ========================================================================

    #[test]
    fn test_z_layers_strictly_ordered() {
        // Grid should be in background
        assert!(Z_LAYER_GRID < Z_LAYER_VOLUME, "Grid should be behind volume");
        assert!(Z_LAYER_VOLUME < Z_LAYER_PANE_BORDERS, "Volume should be behind pane borders");
        assert!(Z_LAYER_PANE_BORDERS < Z_LAYER_RESIZE_GRIPS, "Borders should be behind resize grips");
        assert!(Z_LAYER_RESIZE_GRIPS < Z_LAYER_AXIS_LABELS, "Resize grips should be behind axis labels");
        assert!(Z_LAYER_AXIS_LABELS < Z_LAYER_CROSSHAIR_LINES, "Axis labels should be behind crosshair");
        assert!(Z_LAYER_CROSSHAIR_LINES < Z_LAYER_CROSSHAIR_LABELS, "Crosshair lines should be behind labels");
    }

    // ========================================================================
    // RENDERING DIMENSION TESTS
    // ========================================================================

    #[test]
    fn test_width_ratios_valid() {
        assert!(
            CANDLE_BODY_WIDTH_RATIO > 0.0 && CANDLE_BODY_WIDTH_RATIO <= 1.0,
            "Candle body width ratio must be in (0, 1]"
        );
        assert!(
            VOLUME_BAR_WIDTH_RATIO > 0.0 && VOLUME_BAR_WIDTH_RATIO <= 1.0,
            "Volume bar width ratio must be in (0, 1]"
        );
    }

    #[test]
    fn test_min_element_height_positive() {
        assert!(MIN_ELEMENT_HEIGHT > 0.0, "Min element height must be positive");
    }

    #[test]
    fn test_crosshair_pattern_positive() {
        assert!(CROSSHAIR_DASH_LENGTH > 0.0, "Crosshair dash length must be positive");
        assert!(CROSSHAIR_GAP_LENGTH > 0.0, "Crosshair gap length must be positive");
        assert!(CROSSHAIR_LINE_THICKNESS > 0.0, "Crosshair line thickness must be positive");
    }

    #[test]
    fn test_font_sizes_positive() {
        assert!(AXIS_LABEL_FONT_SIZE > 0.0, "Axis label font size must be positive");
        assert!(CROSSHAIR_LABEL_FONT_SIZE > 0.0, "Crosshair label font size must be positive");
        assert!(OHLCV_BOX_FONT_SIZE > 0.0, "OHLCV box font size must be positive");
        assert!(FPS_COUNTER_FONT_SIZE > 0.0, "FPS counter font size must be positive");
    }

    // ========================================================================
    // TRANSPARENCY VALUE TESTS
    // ========================================================================

    #[test]
    fn test_alpha_values_in_range() {
        let alpha_values = [
            ("volume bar", VOLUME_BAR_ALPHA),
            ("pane border", PANE_BORDER_ALPHA),
            ("crosshair line", CROSSHAIR_LINE_ALPHA),
            ("grid line", GRID_LINE_ALPHA),
            ("resize grip active", RESIZE_GRIP_ACTIVE_ALPHA),
            ("resize grip inactive", RESIZE_GRIP_INACTIVE_ALPHA),
        ];

        for (name, alpha) in alpha_values {
            assert!(
                alpha > 0.0 && alpha <= 1.0,
                "{} alpha ({}) must be in (0, 1]",
                name, alpha
            );
        }
    }

    // ========================================================================
    // DATA LOADING TESTS
    // ========================================================================

    #[test]
    fn test_lazy_load_threshold_positive() {
        assert!(LAZY_LOAD_THRESHOLD > 0, "Lazy load threshold must be positive");
    }

    #[test]
    fn test_lazy_load_batch_size_positive() {
        assert!(LAZY_LOAD_BATCH_SIZE > 0, "Lazy load batch size must be positive");
    }

    // ========================================================================
    // INDICATOR DEFAULTS TESTS
    // ========================================================================

    #[test]
    fn test_sma_periods_ascending() {
        assert!(
            DEFAULT_SMA_SHORT_PERIOD < DEFAULT_SMA_MEDIUM_PERIOD,
            "Short SMA period should be less than medium"
        );
        assert!(
            DEFAULT_SMA_MEDIUM_PERIOD < DEFAULT_SMA_LONG_PERIOD,
            "Medium SMA period should be less than long"
        );
    }

    #[test]
    fn test_sma_periods_positive() {
        assert!(DEFAULT_SMA_SHORT_PERIOD > 0, "Short SMA period must be positive");
        assert!(DEFAULT_SMA_MEDIUM_PERIOD > 0, "Medium SMA period must be positive");
        assert!(DEFAULT_SMA_LONG_PERIOD > 0, "Long SMA period must be positive");
    }

    // ========================================================================
    // GRID CONFIGURATION TESTS
    // ========================================================================

    #[test]
    fn test_tick_counts_positive() {
        assert!(DEFAULT_Y_TICK_COUNT > 0, "Y tick count must be positive");
        assert!(DEFAULT_X_TICK_COUNT > 0, "X tick count must be positive");
    }

    // ========================================================================
    // PRICE PADDING TEST
    // ========================================================================

    #[test]
    fn test_price_padding_reasonable() {
        assert!(
            PRICE_PADDING_PERCENT >= 0.0 && PRICE_PADDING_PERCENT < 1.0,
            "Price padding percent ({}) must be in [0, 1)",
            PRICE_PADDING_PERCENT
        );
    }
}
