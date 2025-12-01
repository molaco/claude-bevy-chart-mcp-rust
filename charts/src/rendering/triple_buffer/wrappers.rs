//! Bevy Resource wrappers for triple-buffered rendering.
//!
//! This module provides the Bevy Resource types that wrap `TripleBufferedResources`
//! for use in the render world.

use bevy::prelude::*;

use super::resources::TripleBufferedResources;

/// Render world resource for triple-buffered candlestick rendering
///
/// This replaces `CandleRenderData` with triple-buffered instance buffers
/// while maintaining shared uniform buffers.
#[derive(Resource, Default)]
pub struct CandleTripleBuffer {
    /// Triple-buffered resources for candlestick instances
    pub resources: TripleBufferedResources,
}

impl CandleTripleBuffer {
    /// Creates a new triple-buffered candlestick resource
    ///
    /// In debug builds, debug stats are enabled by default to help
    /// catch synchronization issues early.
    pub fn new() -> Self {
        #[cfg(debug_assertions)]
        {
            let mut buffer = Self::default();
            buffer.resources.enable_debug();
            buffer
        }
        #[cfg(not(debug_assertions))]
        {
            Self::default()
        }
    }
}

/// Render world resource for triple-buffered volume rendering
///
/// This replaces `VolumeRenderData` with triple-buffered instance buffers
/// while maintaining shared uniform buffers.
#[derive(Resource, Default)]
pub struct VolumeTripleBuffer {
    /// Triple-buffered resources for volume instances
    pub resources: TripleBufferedResources,
}

impl VolumeTripleBuffer {
    /// Creates a new triple-buffered volume resource
    ///
    /// In debug builds, debug stats are enabled by default to help
    /// catch synchronization issues early.
    pub fn new() -> Self {
        #[cfg(debug_assertions)]
        {
            let mut buffer = Self::default();
            buffer.resources.enable_debug();
            buffer
        }
        #[cfg(not(debug_assertions))]
        {
            Self::default()
        }
    }
}
