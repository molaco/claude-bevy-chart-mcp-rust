//! Charts library - Bevy-based candlestick charting library.
//!
//! This library provides all the core functionality for rendering
//! candlestick charts with Bevy, including:
//! - Coordinate transformations between chart data and screen space
//! - Multi-pane layout management
//! - Interaction handlers for pan, zoom, and resize
//! - Technical indicators (SMA, EMA)
//! - GPU-accelerated rendering

pub mod config;
pub mod coordinate;
pub mod data;
pub mod domain;
pub mod indicators;
pub mod interaction;
pub mod panes;
pub mod rendering;
pub mod types;

// Re-export commonly used types for convenience
pub use config::{ChartDimensions, ChartTheme, InteractionConfig, ZLayerConfig};
pub use coordinate::{ChartSpace, ViewportState};
pub use data::{CandleData, ChartDatabase};
pub use domain::{Candle, Timeframe};
pub use indicators::{IndicatorState, MovingAverage};
pub use interaction::{InteractionMode, InteractionState};
pub use panes::{Pane, PaneId, PaneManager, PaneType};
pub use types::ChartMetadata;
