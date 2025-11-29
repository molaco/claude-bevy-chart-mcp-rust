//! Rendering module for GPU-based chart visualization
//!
//! This module contains rendering systems and GPU-compatible data structures
//! for high-performance candlestick chart rendering using instanced drawing.

pub mod instancing;
pub mod instanced_plugin;
pub mod systems;

// Re-export key types for convenience
pub use instancing::{CandleInstance, ViewUniform};

// Re-export plugin and resources
pub use instanced_plugin::{
    CandlestickInstancedPlugin,
    InstancingEnabled,
    ExtractedCandlesInstanced,
    CandleRenderData,
    CandlePipeline,
    CandlestickInstancedLabel,
    CandlestickNode,
};

// Re-export all rendering systems from systems module
pub use systems::*;
