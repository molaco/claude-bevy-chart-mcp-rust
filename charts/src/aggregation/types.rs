use crate::types::Candle;
use bevy::prelude::*;

/// Aggregation level (how many source candles → 1 synthetic candle)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggregationLevel {
    None,      // 1:1 (raw data)
    Low,       // 1:2
    Medium,    // 1:5
    High,      // 1:10
    VeryHigh,  // 1:20
    Extreme,   // 1:50
    Maximum,   // 1:100
}

impl AggregationLevel {
    /// Get the ratio (how many source candles merge into 1)
    pub fn ratio(&self) -> usize {
        match self {
            Self::None => 1,
            Self::Low => 2,
            Self::Medium => 5,
            Self::High => 10,
            Self::VeryHigh => 20,
            Self::Extreme => 50,
            Self::Maximum => 100,
        }
    }

    /// Returns the center offset for positioning aggregated candles
    /// This represents the middle point of the aggregated candle range
    pub fn center_offset(&self) -> usize {
        match self {
            AggregationLevel::None => 0,          // 1:1, no centering needed
            AggregationLevel::Low => 1,           // 2:1, center at index 1 (between 0 and 1)
            AggregationLevel::Medium => 2,        // 5:1, center at index 2 (middle of 0-4)
            AggregationLevel::High => 5,          // 10:1, center at index 5 (middle of 0-9)
            AggregationLevel::VeryHigh => 10,     // 20:1, center at index 10 (middle of 0-19)
            AggregationLevel::Extreme => 25,      // 50:1, center at index 25 (middle of 0-49)
            AggregationLevel::Maximum => 50,      // 100:1, center at index 50 (middle of 0-99)
        }
    }

    /// Select aggregation level based on visible candles and max renderable
    pub fn select(visible_count: usize, max_renderable: usize) -> Self {
        if visible_count <= max_renderable {
            return Self::None;
        }

        let ratio = (visible_count as f32 / max_renderable as f32).ceil() as usize;

        match ratio {
            1 => Self::None,
            2 => Self::Low,
            3..=5 => Self::Medium,
            6..=10 => Self::High,
            11..=20 => Self::VeryHigh,
            21..=50 => Self::Extreme,
            _ => Self::Maximum,
        }
    }
}

/// Container for aggregated candle data at a specific level
#[derive(Debug, Clone)]
pub struct AggregatedCandles {
    pub level: AggregationLevel,
    pub candles: Vec<Candle>,
    pub time_range: (i64, i64),  // Was: source_range: (usize, usize)
}

/// Configuration for the aggregation system
#[derive(Resource, Clone)]
pub struct AggregationConfig {
    pub enabled: bool,
    pub max_renderable_candles: usize,
    pub cache_size_mb: usize,
    pub warmup_on_startup: bool,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_renderable_candles: 1500,
            cache_size_mb: 50,
            warmup_on_startup: true,
        }
    }
}
