//! Synchronization primitives for triple buffering.
//!
//! This module provides types for tracking GPU/CPU synchronization state.

use std::time::Instant;

use super::constants::FRAMES_UNTIL_SAFE;

/// Synchronization state for a frame slot
///
/// Tracks the lifecycle of GPU work associated with a buffer slot.
/// This provides logical synchronization tracking even though wgpu/Bevy
/// handle actual GPU synchronization internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameSyncState {
    /// Slot has never been used or was just created
    #[default]
    Unused,

    /// CPU is currently writing to this slot
    Writing,

    /// Slot has been submitted to GPU, waiting for completion
    /// Contains the frame number when it was submitted
    Submitted,

    /// GPU work is complete, slot is safe for CPU to reuse
    Complete,
}

impl FrameSyncState {
    /// Returns true if the slot is safe for CPU to write to
    pub fn is_safe_for_write(&self) -> bool {
        matches!(self, Self::Unused | Self::Complete)
    }

    /// Returns true if the slot is currently in use by GPU
    pub fn is_gpu_busy(&self) -> bool {
        matches!(self, Self::Submitted)
    }
}

/// Per-frame fence tracking for synchronization
///
/// Tracks when a frame slot was last used and provides methods to determine
/// if the GPU has finished with the associated buffer. While wgpu handles
/// actual synchronization, this provides logical tracking for debugging
/// and ensures correct triple buffer rotation.
///
/// # Synchronization Model
///
/// ```text
/// Frame N:   CPU writes to Slot[N%3], signals fence[N%3]
/// Frame N+1: GPU may still be reading Slot[N%3]
/// Frame N+2: GPU guaranteed done with Slot[N%3] (assuming normal frame pacing)
/// Frame N+3: CPU can safely reuse Slot[N%3]
/// ```
#[derive(Debug, Clone, Default)]
pub struct FrameFence {
    /// The frame number when this slot was last submitted to GPU
    /// None if slot has never been used
    pub submission_frame: Option<u64>,

    /// Current synchronization state
    pub state: FrameSyncState,

    /// Timestamp when submission occurred (for performance monitoring)
    pub submission_time: Option<Instant>,

    /// Timestamp when fence was signaled complete (for latency tracking)
    pub completion_time: Option<Instant>,
}

impl FrameFence {
    /// Creates a new unused fence
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks this fence as submitted at the given frame
    ///
    /// Call this after writing data to the buffer and submitting GPU commands.
    pub fn signal_submission(&mut self, frame: u64) {
        self.submission_frame = Some(frame);
        self.state = FrameSyncState::Submitted;
        self.submission_time = Some(Instant::now());
        self.completion_time = None;
    }

    /// Marks this fence as complete
    ///
    /// Call this when the GPU is guaranteed to be done with this buffer.
    /// With triple buffering, this is typically 2 frames after submission.
    pub fn signal_complete(&mut self) {
        self.state = FrameSyncState::Complete;
        self.completion_time = Some(Instant::now());
    }

    /// Marks this fence as being written to by CPU
    pub fn signal_writing(&mut self) {
        self.state = FrameSyncState::Writing;
    }

    /// Checks if this fence is ready for reuse based on frame count
    ///
    /// # Arguments
    /// * `current_frame` - The current frame number
    ///
    /// # Returns
    /// `true` if enough frames have passed since submission
    pub fn is_ready(&self, current_frame: u64) -> bool {
        match self.submission_frame {
            None => true, // Never used, always ready
            Some(submitted) => {
                // Safe if FRAMES_UNTIL_SAFE frames have passed
                current_frame.saturating_sub(submitted) >= FRAMES_UNTIL_SAFE
            }
        }
    }

    /// Returns the number of frames since this slot was submitted
    ///
    /// Returns None if the slot was never submitted.
    pub fn frames_since_submission(&self, current_frame: u64) -> Option<u64> {
        self.submission_frame
            .map(|submitted| current_frame.saturating_sub(submitted))
    }

    /// Returns the GPU latency if both submission and completion times are recorded
    pub fn measured_latency(&self) -> Option<std::time::Duration> {
        match (self.submission_time, self.completion_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }

    /// Resets the fence to unused state
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
