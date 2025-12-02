//! Rendering module for GPU-based chart visualization
//!
//! This module contains rendering systems and GPU-compatible data structures
//! for high-performance candlestick chart rendering using instanced drawing.

pub mod instancing;
pub mod instanced_plugin;
pub mod volume_instanced_plugin;
pub mod systems;
pub mod triple_buffer;

// Re-export key types for convenience
pub use instancing::{CandleInstance, CandleStyle, ChartConfig, ViewUniform, VolumeInstance};

// Re-export plugin and resources
pub use instanced_plugin::{
    CandlestickInstancedPlugin,
    InstancingEnabled,
    ExtractedCandlesInstanced,
    CandlePipeline,
    CandlestickInstancedLabel,
    CandlestickNode,
};

// Re-export volume plugin
pub use volume_instanced_plugin::{
    VolumeInstancedPlugin,
    ExtractedVolumesInstanced,
    VolumePipeline,
    VolumeInstancedLabel,
    VolumeNode,
};

// Re-export all rendering systems from systems module
pub use systems::*;

// Re-export triple buffering infrastructure
pub use triple_buffer::{
    CandleTripleBuffer,
    VolumeTripleBuffer,
    TripleBufferedResources,
    FrameSlot,
    FrameFence,
    FrameSyncState,
    FrameContext,
    SlotAcquisition,
    RotationState,
    BUFFER_COUNT,
    FRAMES_UNTIL_SAFE,
};
