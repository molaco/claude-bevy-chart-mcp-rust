//! CandleData - Data store for loaded candle data.

use bevy::prelude::Resource;
use crate::domain::Candle;

/// Candle data resource - holds all loaded candle data.
///
/// This resource stores the candles that have been loaded from the database.
/// It supports lazy loading by tracking a global offset for the data window.
#[derive(Resource)]
pub struct CandleData {
    /// All loaded candles
    pub candles: Vec<Candle>,
    /// Global offset for lazy loading (tracks position in full dataset)
    pub candle_offset: usize,
}

impl Default for CandleData {
    fn default() -> Self {
        Self {
            candles: Vec::new(),
            candle_offset: 0,
        }
    }
}

impl CandleData {
    /// Create a new CandleData with the given candles.
    pub fn new(candles: Vec<Candle>) -> Self {
        Self {
            candles,
            candle_offset: 0,
        }
    }

    /// Create a new CandleData with the given candles and offset.
    pub fn with_offset(candles: Vec<Candle>, offset: usize) -> Self {
        Self {
            candles,
            candle_offset: offset,
        }
    }

    /// Get the number of loaded candles.
    pub fn len(&self) -> usize {
        self.candles.len()
    }

    /// Check if there are no loaded candles.
    pub fn is_empty(&self) -> bool {
        self.candles.is_empty()
    }

    /// Get the first candle (earliest in time).
    pub fn first(&self) -> Option<&Candle> {
        self.candles.first()
    }

    /// Get the last candle (most recent in time).
    pub fn last(&self) -> Option<&Candle> {
        self.candles.last()
    }

    /// Get the time range of loaded candles.
    pub fn time_range(&self) -> Option<(i64, i64)> {
        match (self.first(), self.last()) {
            (Some(first), Some(last)) => Some((first.time, last.time)),
            _ => None,
        }
    }
}
