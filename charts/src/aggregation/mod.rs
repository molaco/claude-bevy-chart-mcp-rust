mod engine;
mod types;
mod cache;
mod state;

pub use engine::{aggregate_candles, aggregate_range, aggregate_time_range};
pub use types::{AggregationLevel, AggregatedCandles, AggregationConfig};
pub use cache::{AggregationCache, CacheKey, CacheStats};
pub use state::AggregationState;
