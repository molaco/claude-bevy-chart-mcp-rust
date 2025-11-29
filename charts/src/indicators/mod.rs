//! Indicators module - Moving averages and other technical indicators.
//!
//! This module provides technical analysis indicators for candlestick charts.
//! All calculations are designed for efficient incremental updates during
//! lazy loading (append/prepend operations).
//!
//! # Contents
//!
//! - [`MovingAverage`]: SMA and EMA indicator calculations
//! - [`IndicatorState`]: Bevy resource managing multiple indicators

mod moving_average;

pub use moving_average::MovingAverage;

use bevy::prelude::*;

/// Indicator state resource - manages moving average indicators.
///
/// This resource holds all indicators and provides methods for
/// toggling visibility and iterating over visible indicators.
#[derive(Resource)]
pub struct IndicatorState {
    pub indicators: Vec<MovingAverage>,
}

impl Default for IndicatorState {
    fn default() -> Self {
        Self {
            indicators: Vec::new(),
        }
    }
}

impl IndicatorState {
    pub fn new(indicators: Vec<MovingAverage>) -> Self {
        Self { indicators }
    }

    /// Iterate over visible indicators only.
    pub fn visible_indicators(&self) -> impl Iterator<Item = &MovingAverage> {
        self.indicators.iter().filter(|ma| ma.visible)
    }

    /// Toggle visibility of an indicator by index.
    pub fn toggle_visibility(&mut self, index: usize) {
        if let Some(ma) = self.indicators.get_mut(index) {
            ma.visible = !ma.visible;
        }
    }

    /// Toggle all indicators on/off.
    pub fn toggle_all(&mut self) {
        let any_visible = self.indicators.iter().any(|ma| ma.visible);
        let new_state = !any_visible;
        for ma in self.indicators.iter_mut() {
            ma.visible = new_state;
        }
    }
}
