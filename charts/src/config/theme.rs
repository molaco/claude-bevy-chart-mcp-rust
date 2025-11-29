//! Chart theme configuration with all color definitions.
//!
//! This module provides a unified `ChartTheme` resource that consolidates
//! ALL color configurations in one place, making it easy to create custom
//! themes or switch between light/dark modes.

use bevy::prelude::*;

/// Unified chart theme resource containing all color configurations.
///
/// This resource serves as the single source of truth for all colors
/// used in chart rendering. It extends the existing `ChartColors` concept
/// to include volume bars, UI elements, indicators, and crosshair colors.
#[derive(Resource, Clone)]
pub struct ChartTheme {
    // ========== CANDLESTICK COLORS ==========
    /// Bullish (green) candle body color
    pub bull_candle: Color,

    /// Bearish (red) candle body color
    pub bear_candle: Color,

    /// Candlestick wick color (typically gray)
    pub wick: Color,

    // ========== VOLUME BAR COLORS ==========
    /// Bullish volume bar color (with transparency)
    pub bull_volume: Color,

    /// Bearish volume bar color (with transparency)
    pub bear_volume: Color,

    // ========== GRID COLORS ==========
    /// Grid line color (with transparency)
    pub grid_line: Color,

    // ========== AXIS COLORS ==========
    /// Axis label text color
    pub axis_label: Color,

    // ========== CROSSHAIR COLORS ==========
    /// Crosshair line color
    pub crosshair_line: Color,

    /// Crosshair price/time label background color (yellow)
    pub crosshair_label: Color,

    /// OHLCV info box text color (white)
    pub ohlcv_text: Color,

    // ========== UI ELEMENT COLORS ==========
    /// Pane border color
    pub pane_border: Color,

    /// Resize grip color when active (hovering/dragging)
    pub resize_grip_active: Color,

    /// Resize grip color when inactive
    pub resize_grip_inactive: Color,

    // ========== INDICATOR COLORS ==========
    /// SMA-20 line color (short-term)
    pub sma_short: Color,

    /// SMA-50 line color (medium-term)
    pub sma_medium: Color,

    /// SMA-200 line color (long-term)
    pub sma_long: Color,

    // ========== UTILITY COLORS ==========
    /// FPS counter text color
    pub fps_text: Color,

    /// Background color (for potential future use)
    pub background: Color,
}

impl Default for ChartTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl ChartTheme {
    /// Create the default dark theme
    pub fn dark() -> Self {
        Self {
            // Candlestick colors
            bull_candle: Color::srgb(0.0, 0.8, 0.2),      // Green
            bear_candle: Color::srgb(0.9, 0.2, 0.2),      // Red
            wick: Color::srgb(0.5, 0.5, 0.5),             // Gray

            // Volume bar colors (with transparency)
            bull_volume: Color::srgba(0.0, 0.8, 0.2, 0.6),  // Green, 60% opacity
            bear_volume: Color::srgba(0.9, 0.2, 0.2, 0.6),  // Red, 60% opacity

            // Grid color
            grid_line: Color::srgba(0.3, 0.3, 0.3, 0.3),    // Dark gray, 30% opacity

            // Axis labels
            axis_label: Color::srgb(0.8, 0.8, 0.8),         // Light gray

            // Crosshair colors
            crosshair_line: Color::srgba(1.0, 1.0, 1.0, 0.6),  // White, 60% opacity
            crosshair_label: Color::srgb(1.0, 1.0, 0.0),       // Yellow
            ohlcv_text: Color::srgb(1.0, 1.0, 1.0),            // White

            // UI element colors
            pane_border: Color::srgba(0.5, 0.5, 0.5, 0.6),     // Gray, 60% opacity
            resize_grip_active: Color::srgba(0.9, 0.9, 0.9, 0.9),   // Bright white
            resize_grip_inactive: Color::srgba(0.6, 0.6, 0.6, 0.7), // Dim gray

            // Indicator colors
            sma_short: Color::srgb(1.0, 0.8, 0.0),   // Yellow (SMA-20)
            sma_medium: Color::srgb(0.0, 1.0, 1.0),  // Cyan (SMA-50)
            sma_long: Color::srgb(1.0, 0.0, 1.0),    // Magenta (SMA-200)

            // Utility colors
            fps_text: Color::srgb(0.0, 1.0, 0.0),    // Green
            background: Color::srgb(0.1, 0.1, 0.1),  // Very dark gray
        }
    }

    /// Create a light theme (potential future use)
    pub fn light() -> Self {
        Self {
            // Candlestick colors (slightly darker for light background)
            bull_candle: Color::srgb(0.0, 0.6, 0.15),     // Darker green
            bear_candle: Color::srgb(0.8, 0.15, 0.15),    // Darker red
            wick: Color::srgb(0.3, 0.3, 0.3),             // Darker gray

            // Volume bar colors
            bull_volume: Color::srgba(0.0, 0.6, 0.15, 0.5),
            bear_volume: Color::srgba(0.8, 0.15, 0.15, 0.5),

            // Grid color (darker for light background)
            grid_line: Color::srgba(0.5, 0.5, 0.5, 0.3),

            // Axis labels (dark for light background)
            axis_label: Color::srgb(0.2, 0.2, 0.2),

            // Crosshair colors
            crosshair_line: Color::srgba(0.0, 0.0, 0.0, 0.5),  // Black
            crosshair_label: Color::srgb(0.8, 0.6, 0.0),       // Dark yellow/orange
            ohlcv_text: Color::srgb(0.0, 0.0, 0.0),            // Black

            // UI element colors
            pane_border: Color::srgba(0.3, 0.3, 0.3, 0.6),
            resize_grip_active: Color::srgba(0.2, 0.2, 0.2, 0.9),
            resize_grip_inactive: Color::srgba(0.4, 0.4, 0.4, 0.7),

            // Indicator colors (slightly darker)
            sma_short: Color::srgb(0.8, 0.6, 0.0),    // Dark yellow
            sma_medium: Color::srgb(0.0, 0.7, 0.7),   // Dark cyan
            sma_long: Color::srgb(0.8, 0.0, 0.8),     // Dark magenta

            // Utility colors
            fps_text: Color::srgb(0.0, 0.6, 0.0),     // Dark green
            background: Color::srgb(0.95, 0.95, 0.95), // Very light gray
        }
    }
}

/// Dimension configuration resource for all pixel-based measurements.
///
/// This resource consolidates dimension-related constants that may need
/// to be adjusted for different screen sizes or user preferences.
#[derive(Resource, Clone)]
pub struct ChartDimensions {
    // ========== WINDOW & VIEWPORT ==========
    /// Default window width in pixels
    pub window_width: f32,

    /// Default window height in pixels
    pub window_height: f32,

    /// Chart area width
    pub chart_area_width: f32,

    /// Chart area height
    pub chart_area_height: f32,

    /// Chart area X offset from center
    pub chart_area_offset_x: f32,

    /// Chart area Y offset from center
    pub chart_area_offset_y: f32,

    // ========== PANE LAYOUT ==========
    /// Gap between panes (separator/resize area)
    pub separator_gap: f32,

    /// Border thickness
    pub border_thickness: f32,

    // ========== RENDERING ==========
    /// Body width ratio (fraction of candle spacing)
    pub body_width_ratio: f32,

    /// Minimum element height
    pub min_element_height: f32,

    // ========== CROSSHAIR ==========
    /// Dash length for crosshair pattern
    pub crosshair_dash_length: f32,

    /// Gap length for crosshair pattern
    pub crosshair_gap_length: f32,

    // ========== LABEL OFFSETS ==========
    /// X offset for price labels
    pub price_label_offset_x: f32,

    /// Y offset for time labels
    pub time_label_offset_y: f32,

    /// OHLCV box X offset
    pub ohlcv_box_offset_x: f32,

    /// OHLCV box Y offset
    pub ohlcv_box_offset_y: f32,

    // ========== FONT SIZES ==========
    /// Axis label font size
    pub axis_label_font_size: f32,

    /// Crosshair label font size
    pub crosshair_label_font_size: f32,

    /// OHLCV box font size
    pub ohlcv_box_font_size: f32,

    /// FPS counter font size
    pub fps_font_size: f32,

    // ========== RESIZE GRIP ==========
    /// Resize grip width
    pub resize_grip_width: f32,

    /// Resize grip line height
    pub resize_grip_line_height: f32,

    /// Resize grip line spacing
    pub resize_grip_line_spacing: f32,
}

impl Default for ChartDimensions {
    fn default() -> Self {
        use super::constants::*;

        Self {
            window_width: WINDOW_WIDTH,
            window_height: WINDOW_HEIGHT,
            chart_area_width: CHART_AREA_WIDTH,
            chart_area_height: CHART_AREA_HEIGHT,
            chart_area_offset_x: CHART_AREA_OFFSET_X,
            chart_area_offset_y: CHART_AREA_OFFSET_Y,
            separator_gap: PANE_SEPARATOR_GAP,
            border_thickness: BORDER_THICKNESS,
            body_width_ratio: CANDLE_BODY_WIDTH_RATIO,
            min_element_height: MIN_ELEMENT_HEIGHT,
            crosshair_dash_length: CROSSHAIR_DASH_LENGTH,
            crosshair_gap_length: CROSSHAIR_GAP_LENGTH,
            price_label_offset_x: PRICE_LABEL_OFFSET_X,
            time_label_offset_y: TIME_LABEL_OFFSET_Y,
            ohlcv_box_offset_x: OHLCV_BOX_OFFSET_X,
            ohlcv_box_offset_y: OHLCV_BOX_OFFSET_Y,
            axis_label_font_size: AXIS_LABEL_FONT_SIZE,
            crosshair_label_font_size: CROSSHAIR_LABEL_FONT_SIZE,
            ohlcv_box_font_size: OHLCV_BOX_FONT_SIZE,
            fps_font_size: FPS_COUNTER_FONT_SIZE,
            resize_grip_width: RESIZE_GRIP_WIDTH,
            resize_grip_line_height: RESIZE_GRIP_LINE_HEIGHT,
            resize_grip_line_spacing: RESIZE_GRIP_LINE_SPACING,
        }
    }
}

/// Interaction configuration resource for zoom, pan, and resize behavior.
#[derive(Resource, Clone)]
pub struct InteractionConfig {
    // ========== ZOOM ==========
    /// Zoom factor when scrolling up (zoom in)
    pub zoom_in_factor: f32,

    /// Zoom factor when scrolling down (zoom out)
    pub zoom_out_factor: f32,

    /// Minimum number of visible candles
    pub min_visible_candles: f32,

    /// Maximum number of visible candles
    pub max_visible_candles: f32,

    /// Default number of visible candles
    pub default_visible_candles: usize,

    // ========== PANE RESIZE ==========
    /// Minimum pane height as a percentage (0.0-1.0)
    pub min_pane_height: f32,

    /// Maximum pane height as a percentage (0.0-1.0)
    pub max_pane_height: f32,

    // ========== LAZY LOADING ==========
    /// Threshold in candles from edge to trigger lazy loading
    pub lazy_load_threshold: usize,

    /// Number of candles to load per request
    pub lazy_load_batch_size: i64,

    // ========== PANE DEFAULTS ==========
    /// Default price pane height percentage
    pub default_price_pane_height: f32,

    /// Default volume pane height percentage
    pub default_volume_pane_height: f32,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        use super::constants::*;

        Self {
            zoom_in_factor: ZOOM_IN_FACTOR,
            zoom_out_factor: ZOOM_OUT_FACTOR,
            min_visible_candles: MIN_VISIBLE_CANDLES,
            max_visible_candles: MAX_VISIBLE_CANDLES,
            default_visible_candles: DEFAULT_VISIBLE_CANDLES,
            min_pane_height: MIN_PANE_HEIGHT_PERCENT,
            max_pane_height: MAX_PANE_HEIGHT_PERCENT,
            lazy_load_threshold: LAZY_LOAD_THRESHOLD,
            lazy_load_batch_size: LAZY_LOAD_BATCH_SIZE,
            default_price_pane_height: DEFAULT_PRICE_PANE_HEIGHT,
            default_volume_pane_height: DEFAULT_VOLUME_PANE_HEIGHT,
        }
    }
}

/// Z-layer ordering configuration for proper element stacking.
#[derive(Resource, Clone)]
pub struct ZLayerConfig {
    /// Grid lines (background)
    pub grid: f32,

    /// Volume bars
    pub volume: f32,

    /// Pane borders
    pub pane_borders: f32,

    /// Resize grips
    pub resize_grips: f32,

    /// Axis labels
    pub axis_labels: f32,

    /// Crosshair lines
    pub crosshair_lines: f32,

    /// Crosshair labels
    pub crosshair_labels: f32,
}

impl Default for ZLayerConfig {
    fn default() -> Self {
        use super::constants::*;

        Self {
            grid: Z_LAYER_GRID,
            volume: Z_LAYER_VOLUME,
            pane_borders: Z_LAYER_PANE_BORDERS,
            resize_grips: Z_LAYER_RESIZE_GRIPS,
            axis_labels: Z_LAYER_AXIS_LABELS,
            crosshair_lines: Z_LAYER_CROSSHAIR_LINES,
            crosshair_labels: Z_LAYER_CROSSHAIR_LABELS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to extract alpha from Color (works with srgb/srgba)
    fn get_alpha(color: &Color) -> f32 {
        match color {
            Color::Srgba(srgba) => srgba.alpha,
            _ => 1.0, // Default to fully opaque for other color types
        }
    }

    // ========================================================================
    // CHART THEME TESTS
    // ========================================================================

    #[test]
    fn test_chart_theme_dark_creates_valid_theme() {
        let theme = ChartTheme::dark();

        // Verify all colors can be accessed without panic
        let _ = theme.bull_candle;
        let _ = theme.bear_candle;
        let _ = theme.wick;
        let _ = theme.bull_volume;
        let _ = theme.bear_volume;
        let _ = theme.grid_line;
        let _ = theme.axis_label;
        let _ = theme.crosshair_line;
        let _ = theme.crosshair_label;
        let _ = theme.ohlcv_text;
        let _ = theme.pane_border;
        let _ = theme.resize_grip_active;
        let _ = theme.resize_grip_inactive;
        let _ = theme.sma_short;
        let _ = theme.sma_medium;
        let _ = theme.sma_long;
        let _ = theme.fps_text;
        let _ = theme.background;
    }

    #[test]
    fn test_chart_theme_light_creates_valid_theme() {
        let theme = ChartTheme::light();

        // Verify all colors can be accessed without panic
        let _ = theme.bull_candle;
        let _ = theme.bear_candle;
        let _ = theme.wick;
        let _ = theme.bull_volume;
        let _ = theme.bear_volume;
        let _ = theme.grid_line;
    }

    #[test]
    fn test_chart_theme_default_is_dark() {
        let default_theme = ChartTheme::default();
        let dark_theme = ChartTheme::dark();

        // Default should match dark theme (compare a few key colors)
        assert_eq!(
            format!("{:?}", default_theme.bull_candle),
            format!("{:?}", dark_theme.bull_candle),
            "Default theme should use dark theme colors"
        );
    }

    #[test]
    fn test_chart_theme_dark_colors_visible() {
        let theme = ChartTheme::dark();

        // Colors that should be fully opaque
        let opaque_colors = [
            ("bull_candle", &theme.bull_candle),
            ("bear_candle", &theme.bear_candle),
            ("wick", &theme.wick),
            ("crosshair_label", &theme.crosshair_label),
            ("ohlcv_text", &theme.ohlcv_text),
            ("sma_short", &theme.sma_short),
            ("sma_medium", &theme.sma_medium),
            ("sma_long", &theme.sma_long),
        ];

        for (name, color) in opaque_colors {
            let alpha = get_alpha(color);
            assert!(
                alpha > 0.9,
                "{} should be nearly opaque, got alpha {}",
                name, alpha
            );
        }
    }

    #[test]
    fn test_chart_theme_dark_transparent_colors_have_some_alpha() {
        let theme = ChartTheme::dark();

        // Colors that should have transparency but still be visible
        let transparent_colors = [
            ("bull_volume", &theme.bull_volume),
            ("bear_volume", &theme.bear_volume),
            ("grid_line", &theme.grid_line),
            ("crosshair_line", &theme.crosshair_line),
            ("pane_border", &theme.pane_border),
        ];

        for (name, color) in transparent_colors {
            let alpha = get_alpha(color);
            assert!(
                alpha > 0.0,
                "{} should have positive alpha, got {}",
                name, alpha
            );
        }
    }

    #[test]
    fn test_chart_theme_light_colors_visible() {
        let theme = ChartTheme::light();

        // Key colors should be visible
        let key_colors = [
            ("bull_candle", &theme.bull_candle),
            ("bear_candle", &theme.bear_candle),
        ];

        for (name, color) in key_colors {
            let alpha = get_alpha(color);
            assert!(
                alpha > 0.9,
                "Light theme {} should be nearly opaque, got alpha {}",
                name, alpha
            );
        }
    }

    // ========================================================================
    // CHART DIMENSIONS TESTS
    // ========================================================================

    #[test]
    fn test_chart_dimensions_default_all_positive() {
        let dims = ChartDimensions::default();

        assert!(dims.window_width > 0.0, "Window width must be positive");
        assert!(dims.window_height > 0.0, "Window height must be positive");
        assert!(dims.chart_area_width > 0.0, "Chart area width must be positive");
        assert!(dims.chart_area_height > 0.0, "Chart area height must be positive");
        assert!(dims.separator_gap > 0.0, "Separator gap must be positive");
        assert!(dims.border_thickness > 0.0, "Border thickness must be positive");
        assert!(dims.body_width_ratio > 0.0, "Body width ratio must be positive");
        assert!(dims.min_element_height > 0.0, "Min element height must be positive");
        assert!(dims.crosshair_dash_length > 0.0, "Crosshair dash length must be positive");
        assert!(dims.crosshair_gap_length > 0.0, "Crosshair gap length must be positive");
        assert!(dims.axis_label_font_size > 0.0, "Axis label font size must be positive");
    }

    #[test]
    fn test_chart_dimensions_chart_area_smaller_than_window() {
        let dims = ChartDimensions::default();

        assert!(
            dims.chart_area_width < dims.window_width,
            "Chart area width should be smaller than window width"
        );
        assert!(
            dims.chart_area_height < dims.window_height,
            "Chart area height should be smaller than window height"
        );
    }

    // ========================================================================
    // INTERACTION CONFIG TESTS
    // ========================================================================

    #[test]
    fn test_interaction_config_zoom_factors_valid() {
        let config = InteractionConfig::default();

        assert!(
            config.zoom_in_factor > 0.0 && config.zoom_in_factor < 1.0,
            "Zoom in factor ({}) must be in (0, 1)",
            config.zoom_in_factor
        );
        assert!(
            config.zoom_out_factor > 1.0,
            "Zoom out factor ({}) must be > 1.0",
            config.zoom_out_factor
        );
    }

    #[test]
    fn test_interaction_config_pane_heights_valid() {
        let config = InteractionConfig::default();

        assert!(
            config.min_pane_height > 0.0,
            "Min pane height must be positive"
        );
        assert!(
            config.max_pane_height <= 1.0,
            "Max pane height must be <= 1.0"
        );
        assert!(
            config.min_pane_height < config.max_pane_height,
            "Min pane height must be less than max"
        );
    }

    #[test]
    fn test_interaction_config_visible_candles_valid() {
        let config = InteractionConfig::default();

        assert!(
            config.min_visible_candles > 0.0,
            "Min visible candles must be positive"
        );
        assert!(
            config.max_visible_candles > config.min_visible_candles,
            "Max visible candles must exceed minimum"
        );
        assert!(
            (config.default_visible_candles as f32) >= config.min_visible_candles,
            "Default visible candles below minimum"
        );
        assert!(
            (config.default_visible_candles as f32) <= config.max_visible_candles,
            "Default visible candles above maximum"
        );
    }

    #[test]
    fn test_interaction_config_default_heights_valid() {
        let config = InteractionConfig::default();

        assert!(
            config.default_price_pane_height >= config.min_pane_height,
            "Default price pane height below minimum"
        );
        assert!(
            config.default_price_pane_height <= config.max_pane_height,
            "Default price pane height above maximum"
        );
        assert!(
            config.default_volume_pane_height >= config.min_pane_height,
            "Default volume pane height below minimum"
        );
        assert!(
            config.default_volume_pane_height <= config.max_pane_height,
            "Default volume pane height above maximum"
        );

        let sum = config.default_price_pane_height + config.default_volume_pane_height;
        assert!(
            (sum - 1.0).abs() < 0.001,
            "Default pane heights should sum to 1.0, got {}",
            sum
        );
    }

    #[test]
    fn test_interaction_config_lazy_load_valid() {
        let config = InteractionConfig::default();

        assert!(
            config.lazy_load_threshold > 0,
            "Lazy load threshold must be positive"
        );
        assert!(
            config.lazy_load_batch_size > 0,
            "Lazy load batch size must be positive"
        );
    }

    // ========================================================================
    // Z-LAYER CONFIG TESTS
    // ========================================================================

    #[test]
    fn test_z_layer_config_ordering() {
        let z = ZLayerConfig::default();

        assert!(z.grid < z.volume, "Grid should be behind volume");
        assert!(z.volume < z.pane_borders, "Volume should be behind pane borders");
        assert!(z.pane_borders < z.resize_grips, "Pane borders should be behind resize grips");
        assert!(z.resize_grips < z.axis_labels, "Resize grips should be behind axis labels");
        assert!(z.axis_labels < z.crosshair_lines, "Axis labels should be behind crosshair lines");
        assert!(z.crosshair_lines < z.crosshair_labels, "Crosshair lines should be behind labels");
    }
}
