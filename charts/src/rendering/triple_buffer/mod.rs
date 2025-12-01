//! Triple buffering infrastructure for GPU resources
//!
//! This module provides the data structures and utilities for implementing
//! triple buffering of GPU resources. Triple buffering allows the CPU to
//! write to one buffer while the GPU reads from another, eliminating
//! synchronization stalls during rendering.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    TRIPLE BUFFER LOOP                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Frame 0: CPU writes Buffer[0], GPU reads Buffer[2]         │
//! │  Frame 1: CPU writes Buffer[1], GPU reads Buffer[0]         │
//! │  Frame 2: CPU writes Buffer[2], GPU reads Buffer[1]         │
//! │  Frame 3: Back to Frame 0's pattern...                      │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Rotation Scheme
//!
//! The rotation follows a simple modulo-3 pattern:
//!
//! ```text
//! Index formula: current_buffer = frame_number % 3
//!
//! Frame │ Write Slot │ GPU Reading │ Safe to Reuse
//! ──────┼────────────┼─────────────┼──────────────
//!   0   │     0      │     2       │     1
//!   1   │     1      │     0       │     2
//!   2   │     2      │     1       │     0
//!   3   │     0      │     2       │     1  (cycle repeats)
//! ```
//!
//! **Key insight**: When writing to slot N, the GPU is reading from slot (N+2)%3,
//! which was written 2 frames ago. Slot (N+1)%3 is "in flight" - submitted but
//! not yet guaranteed complete.
//!
//! # Synchronization Guarantee
//!
//! With proper fence usage:
//! - Slot written at frame F is submitted to GPU at frame F
//! - By frame F+2, GPU work from frame F is guaranteed complete
//! - At frame F+3, slot from frame F can be safely overwritten
//!
//! This means we wait for the fence from 3 frames ago before writing,
//! ensuring no data races between CPU writes and GPU reads.
//!
//! # Usage
//!
//! The `TripleBufferedResources` struct manages three sets of frame resources.
//! Each frame, call `current_index()` to determine which buffer slot to use,
//! then `advance_frame()` after rendering completes.
//!
//! ```ignore
//! // Each frame:
//! let index = resources.current_index();
//! // Wait for fence[index] if using explicit synchronization
//! write_to_buffer(&resources.slots[index]);
//! submit_gpu_commands();
//! // Signal fence[index]
//! resources.advance_frame();
//! ```
//!
//! # Module Structure
//!
//! - `constants`: Buffer count, timing, and capacity constants
//! - `sync`: Synchronization state tracking (FrameSyncState, FrameFence)
//! - `slot`: Per-frame slot containing buffers (FrameSlot)
//! - `debug_stats`: Debug statistics and frame timing (DebugStats, FrameTiming)
//! - `allocation_stats`: Buffer allocation tracking (AllocationStats)
//! - `capacity`: Buffer size calculation utilities
//! - `resources`: Main TripleBufferedResources struct + helper types
//! - `wrappers`: Bevy Resource wrappers (CandleTripleBuffer, VolumeTripleBuffer)

pub mod allocation_stats;
pub mod capacity;
pub mod constants;
pub mod debug_stats;
pub mod resources;
pub mod slot;
pub mod sync;
pub mod wrappers;

#[cfg(test)]
mod tests;

// Re-export all public types for convenience
pub use allocation_stats::AllocationStats;
pub use capacity::{calculate_grown_capacity, calculate_initial_capacity, format_bytes};
pub use constants::{
    BUFFER_COUNT, BUFFER_GROWTH_FACTOR, DEBUG_HISTORY_SIZE, FRAMES_UNTIL_SAFE, MIN_BUFFER_CAPACITY,
};
pub use debug_stats::{DebugStats, FrameTiming};
pub use resources::{FrameContext, RotationState, SlotAcquisition, TripleBufferedResources};
pub use slot::FrameSlot;
pub use sync::{FrameFence, FrameSyncState};
pub use wrappers::{CandleTripleBuffer, VolumeTripleBuffer};
