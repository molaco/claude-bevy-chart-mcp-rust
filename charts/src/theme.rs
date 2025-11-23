//! Centralized theme configuration for Bevy chart rendering.
//!
//! Provides color definitions and utilities for consistent chart styling.

use bevy::prelude::{Color, Resource};
use bevy::color::Srgba;

/// Color definitions for chart rendering.
///
/// This struct holds all the colors used throughout the chart visualization,
/// providing a centralized place for theme customization.
#[derive(Resource, Clone, Debug)]
pub struct ChartColors {
    /// Background color for the chart area
    pub background: Color,
    /// Grid line color
    pub grid: Color,
    /// Text color for labels and values
    pub text: Color,
    /// Bullish (up) candle color - teal green
    pub bull_candle: Color,
    /// Bearish (down) candle color - red
    pub bear_candle: Color,
    /// Crosshair line color
    pub crosshair: Color,
    /// Axis line color
    pub axis_line: Color,
    /// Volume bar color for bullish candles
    pub volume_bull: Color,
    /// Volume bar color for bearish candles
    pub volume_bear: Color,
}

impl Default for ChartColors {
    fn default() -> Self {
        Self {
            // Dark background: #181616
            background: color_from_hex("#181616"),
            // Subtle grid lines
            grid: Color::Srgba(Srgba::new(0.3, 0.3, 0.3, 0.3)),
            // Light gray text: #c5c9c5
            text: color_from_hex("#c5c9c5"),
            // Teal green for bullish candles: #51cda0
            bull_candle: color_from_hex("#51cda0"),
            // Red for bearish candles: #c0504d
            bear_candle: color_from_hex("#c0504d"),
            // Semi-transparent white crosshair
            crosshair: Color::Srgba(Srgba::new(1.0, 1.0, 1.0, 0.6)),
            // Slightly visible axis lines
            axis_line: Color::Srgba(Srgba::new(0.4, 0.4, 0.4, 0.8)),
            // Volume colors with transparency
            volume_bull: Color::Srgba(Srgba::new(0.318, 0.804, 0.627, 0.6)), // #51cda0 with 0.6 alpha
            volume_bear: Color::Srgba(Srgba::new(0.753, 0.314, 0.302, 0.6)), // #c0504d with 0.6 alpha
        }
    }
}

/// Convert a hex color string to a Bevy Color using Srgba format.
///
/// # Arguments
///
/// * `hex` - A hex color string with or without the leading '#' (e.g., "#181616" or "181616")
///
/// # Returns
///
/// A `bevy::prelude::Color` in Srgba format.
///
/// # Example
///
/// ```
/// let color = color_from_hex("#51cda0");
/// ```
pub fn color_from_hex(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

    Color::Srgba(Srgba::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex_with_hash() {
        let color = color_from_hex("#181616");
        if let Color::Srgba(srgba) = color {
            assert!((srgba.red - 0.094).abs() < 0.01);
            assert!((srgba.green - 0.086).abs() < 0.01);
            assert!((srgba.blue - 0.086).abs() < 0.01);
            assert!((srgba.alpha - 1.0).abs() < 0.001);
        } else {
            panic!("Expected Srgba color");
        }
    }

    #[test]
    fn test_color_from_hex_without_hash() {
        let color = color_from_hex("51cda0");
        if let Color::Srgba(srgba) = color {
            assert!((srgba.red - 0.318).abs() < 0.01);
            assert!((srgba.green - 0.804).abs() < 0.01);
            assert!((srgba.blue - 0.627).abs() < 0.01);
        } else {
            panic!("Expected Srgba color");
        }
    }

    #[test]
    fn test_default_colors() {
        let colors = ChartColors::default();
        // Just verify it doesn't panic and creates valid colors
        assert!(matches!(colors.background, Color::Srgba(_)));
        assert!(matches!(colors.bull_candle, Color::Srgba(_)));
        assert!(matches!(colors.bear_candle, Color::Srgba(_)));
    }
}
