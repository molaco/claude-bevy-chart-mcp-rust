//! Frame slot for triple buffered resources.
//!
//! This module provides the `FrameSlot` struct which represents a single
//! buffer slot in the triple buffer rotation.

use bevy::render::render_resource::{BindGroup, Buffer};

use super::sync::{FrameFence, FrameSyncState};

/// Resources for a single frame slot in the triple buffer rotation
///
/// Each frame slot contains its own instance buffer, uniform buffer, and bind group,
/// allowing the CPU to write to one slot while the GPU reads from another.
/// This ensures complete isolation between frames - no shared mutable state.
///
/// # Memory Layout
/// - `instance_buffer`: Vertex buffer containing instance data (e.g., CandleInstance)
/// - `view_uniform_buffer`: Uniform buffer for view/projection data (triple-buffered)
/// - `bind_group`: Pre-created bind group referencing this slot's buffers
/// - `buffer_capacity`: Tracks allocated size to avoid unnecessary reallocations
/// - `fence`: Synchronization tracking for this slot
#[derive(Default)]
pub struct FrameSlot {
    /// GPU buffer containing instance data for this frame slot
    pub instance_buffer: Option<Buffer>,

    /// GPU buffer containing view uniform data for this frame slot
    /// Triple-buffered to avoid CPU/GPU contention on uniform updates
    pub view_uniform_buffer: Option<Buffer>,

    /// Bind group for this frame slot (references this slot's uniform buffer)
    pub bind_group: Option<BindGroup>,

    /// Allocated capacity of instance buffer in bytes
    /// Used to determine if reallocation is needed
    pub buffer_capacity: usize,

    /// Allocated capacity of view uniform buffer in bytes
    pub uniform_buffer_capacity: usize,

    /// Whether this slot's bind group is valid
    /// Set to false when buffers are reallocated
    pub bind_group_valid: bool,

    /// Fence tracking for GPU synchronization
    /// Tracks when this slot was submitted and whether GPU is done
    pub fence: FrameFence,
}

impl FrameSlot {
    /// Creates a new empty frame slot
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if this slot needs instance buffer reallocation
    ///
    /// # Arguments
    /// * `required_size` - The size in bytes needed for instance data
    ///
    /// # Returns
    /// `true` if the current buffer is too small or doesn't exist
    pub fn needs_reallocation(&self, required_size: usize) -> bool {
        self.instance_buffer.is_none() || self.buffer_capacity < required_size
    }

    /// Checks if this slot needs uniform buffer reallocation
    ///
    /// # Arguments
    /// * `required_size` - The size in bytes needed for uniform data
    ///
    /// # Returns
    /// `true` if the current buffer is too small or doesn't exist
    pub fn needs_uniform_reallocation(&self, required_size: usize) -> bool {
        self.view_uniform_buffer.is_none() || self.uniform_buffer_capacity < required_size
    }

    /// Invalidates the bind group, forcing recreation on next use
    pub fn invalidate_bind_group(&mut self) {
        self.bind_group_valid = false;
    }

    /// Checks if this slot is safe for CPU to write to
    ///
    /// A slot is safe to write when its fence indicates the GPU is done
    /// or the slot has never been used.
    pub fn is_safe_for_write(&self, current_frame: u64) -> bool {
        self.fence.is_ready(current_frame)
    }

    /// Begins writing to this slot
    ///
    /// Call this before writing data to the buffer.
    /// Updates fence state to Writing.
    pub fn begin_write(&mut self) {
        self.fence.signal_writing();
    }

    /// Completes writing and marks slot as submitted to GPU
    ///
    /// Call this after writing data and submitting GPU commands.
    pub fn end_write(&mut self, frame: u64) {
        self.fence.signal_submission(frame);
    }

    /// Marks this slot's GPU work as complete
    ///
    /// Call this when we know the GPU is done with this slot's buffer.
    pub fn mark_complete(&mut self) {
        self.fence.signal_complete();
    }

    /// Returns the synchronization state of this slot
    pub fn sync_state(&self) -> FrameSyncState {
        self.fence.state
    }
}
