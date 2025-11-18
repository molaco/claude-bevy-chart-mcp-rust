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
    pub source_range: (usize, usize), // (start_index, end_index) in source data
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
