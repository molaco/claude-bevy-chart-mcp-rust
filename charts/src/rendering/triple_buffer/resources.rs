//! Triple-buffered GPU resources.
//!
//! This module provides the main `TripleBufferedResources` struct that manages
//! three frame slots rotating each frame, along with helper types for rotation
//! state, slot acquisition, and frame context.

use bevy::render::render_resource::Buffer;

use super::allocation_stats::AllocationStats;
use super::capacity::{calculate_grown_capacity, calculate_initial_capacity, format_bytes};
use super::constants::{BUFFER_COUNT, FRAMES_UNTIL_SAFE};
use super::debug_stats::DebugStats;
use super::slot::FrameSlot;
use super::sync::FrameSyncState;

// ==================== Helper Types ====================

/// Describes the current state of buffer rotation for debugging and visualization
///
/// This struct provides a snapshot of which slots are being used for what purpose
/// at any given frame. Useful for debugging synchronization issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationState {
    /// Current frame number
    pub frame: u64,

    /// Index of the slot being written to by CPU this frame
    pub write_slot: usize,

    /// Index of the slot the GPU is currently reading from
    /// (This is the slot that was written 2 frames ago)
    pub gpu_read_slot: usize,

    /// Index of the slot that is "in flight" (submitted but not guaranteed complete)
    /// (This is the slot that was written 1 frame ago)
    pub in_flight_slot: usize,
}

impl RotationState {
    /// Creates a new rotation state for the given frame number
    pub fn for_frame(frame: u64) -> Self {
        let write_slot = (frame % BUFFER_COUNT as u64) as usize;
        // GPU reads from the slot written 2 frames ago
        let gpu_read_slot = ((frame + BUFFER_COUNT as u64 - 2) % BUFFER_COUNT as u64) as usize;
        // In-flight is the slot written 1 frame ago
        let in_flight_slot = ((frame + BUFFER_COUNT as u64 - 1) % BUFFER_COUNT as u64) as usize;

        Self {
            frame,
            write_slot,
            gpu_read_slot,
            in_flight_slot,
        }
    }

    /// Returns true if the given slot index is safe to write to
    ///
    /// A slot is safe to write when it's neither being read by the GPU
    /// nor in-flight from the previous frame.
    pub fn is_safe_to_write(&self, slot_index: usize) -> bool {
        slot_index == self.write_slot
    }

    /// Returns a human-readable description of slot usage
    pub fn slot_status(&self, slot_index: usize) -> &'static str {
        if slot_index == self.write_slot {
            "WRITING"
        } else if slot_index == self.gpu_read_slot {
            "GPU_READ"
        } else if slot_index == self.in_flight_slot {
            "IN_FLIGHT"
        } else {
            "UNKNOWN"
        }
    }
}

impl std::fmt::Display for RotationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Frame {}: Write[{}] InFlight[{}] GPURead[{}]",
            self.frame, self.write_slot, self.in_flight_slot, self.gpu_read_slot
        )
    }
}

/// Result of attempting to acquire a buffer slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAcquisition {
    /// Slot is ready for writing
    Ready {
        /// Index of the ready slot
        slot_index: usize,
    },
    /// Slot is still in use by GPU
    Busy {
        /// Index of the busy slot
        slot_index: usize,
        /// Estimated frames until ready
        frames_remaining: u64,
    },
}

impl SlotAcquisition {
    /// Returns true if the slot is ready
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// Returns the slot index regardless of ready state
    pub fn slot_index(&self) -> usize {
        match self {
            Self::Ready { slot_index } | Self::Busy { slot_index, .. } => *slot_index,
        }
    }
}

/// Context for the current frame's rendering
///
/// Returned by `begin_frame()` to provide all information needed
/// for the current frame's buffer operations.
#[derive(Debug, Clone, Copy)]
pub struct FrameContext {
    /// Current frame number
    pub frame_number: u64,

    /// Index of the slot being used this frame (0, 1, or 2)
    pub slot_index: usize,

    /// Number of instances to render
    pub instance_count: u32,
}

impl FrameContext {
    /// Returns true if there are instances to render
    pub fn has_instances(&self) -> bool {
        self.instance_count > 0
    }
}

// ==================== Main Resource ====================

/// Triple-buffered GPU resources for a single render data type
///
/// Manages three frame slots that rotate each frame, ensuring the CPU
/// can always write to a buffer that the GPU is not currently reading.
///
/// # Synchronization
/// The frame counter determines which slot is active. After 3 frames,
/// a slot's GPU work is guaranteed to be complete (assuming proper fence usage).
///
/// # Triple-Buffered Resources (per-slot)
/// - Instance buffers: Contain vertex instance data
/// - View uniform buffers: Contain projection matrix, viewport, colors
/// - Bind groups: Reference each slot's own buffers
///
/// # Shared Resources
/// - `config_buffer`: Static data, never changes after creation
///
/// # Memory Management (Phase 5)
/// - Buffers are pre-allocated with a growth factor to reduce reallocation frequency
/// - All slots maintain consistent capacity for predictable memory usage
/// - Allocation statistics are tracked for debugging and optimization
#[derive(Default)]
pub struct TripleBufferedResources {
    /// Three frame slots for rotation
    pub slots: [FrameSlot; BUFFER_COUNT],

    /// Global frame counter for determining active slot
    /// Increments each frame, wraps automatically via modulo
    pub frame_count: u64,

    /// Shared config buffer (static, never changes)
    pub config_buffer: Option<Buffer>,

    // Change detection for uniform updates
    /// Last viewport width for change detection
    pub last_viewport_width: f32,

    /// Last viewport height for change detection
    pub last_viewport_height: f32,

    /// Last bullish color for change detection
    pub last_bull_color: [f32; 4],

    /// Last bearish color for change detection
    pub last_bear_color: [f32; 4],

    /// Last wick color for change detection (candlesticks only)
    pub last_wick_color: [f32; 4],

    /// Number of instances currently stored
    pub instance_count: u32,

    /// Statistics for tracking allocation patterns (Phase 5)
    pub allocation_stats: AllocationStats,

    /// Debug statistics for performance monitoring (Phase 6)
    /// Enable with `debug_stats.enable()` for detailed timing info
    pub debug_stats: DebugStats,

    /// Track which slot was written to in current frame.
    /// This is set by prepare (begin_frame) and read by render (render_slot_index).
    /// Ensures render always reads from the slot prepare just wrote to.
    pub current_write_slot: usize,
}

impl TripleBufferedResources {
    /// Creates a new triple-buffered resource container
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new triple-buffered resource container with debug enabled
    #[cfg(debug_assertions)]
    pub fn new_with_debug() -> Self {
        Self {
            debug_stats: DebugStats::new_enabled(),
            ..Default::default()
        }
    }

    /// Checks if buffers have been pre-allocated
    ///
    /// Returns true if all 3 slots have instance buffers allocated.
    pub fn is_preallocated(&self) -> bool {
        self.slots.iter().all(|slot| slot.instance_buffer.is_some())
    }

    /// Returns the index of the current frame slot (0, 1, or 2)
    ///
    /// This is the slot that should be written to by the CPU
    /// and read from by the GPU for the current frame.
    #[inline]
    pub fn current_index(&self) -> usize {
        (self.frame_count % BUFFER_COUNT as u64) as usize
    }

    /// Returns the slot index that render should read from.
    ///
    /// This is the slot that was written to by prepare in the current frame.
    /// By storing this value in `begin_frame()`, we ensure render always reads
    /// from the correct slot regardless of when `complete_frame()` advances
    /// the frame counter.
    #[inline]
    pub fn render_slot_index(&self) -> usize {
        self.current_write_slot
    }

    /// Returns a reference to the current frame slot
    #[inline]
    pub fn current_slot(&self) -> &FrameSlot {
        &self.slots[self.current_index()]
    }

    /// Returns a mutable reference to the current frame slot
    #[inline]
    pub fn current_slot_mut(&mut self) -> &mut FrameSlot {
        let index = self.current_index();
        &mut self.slots[index]
    }

    /// Advances to the next frame, incrementing the frame counter
    ///
    /// Call this at the end of each frame after rendering completes.
    /// The next call to `current_index()` will return the next slot.
    #[inline]
    pub fn advance_frame(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);
    }

    /// Returns the index that was used N frames ago
    ///
    /// Useful for debugging and understanding which buffer the GPU
    /// might still be reading from.
    ///
    /// # Arguments
    /// * `frames_ago` - Number of frames in the past (0 = current)
    #[inline]
    pub fn index_frames_ago(&self, frames_ago: u64) -> usize {
        ((self.frame_count.wrapping_sub(frames_ago)) % BUFFER_COUNT as u64) as usize
    }

    /// Returns the current rotation state for debugging
    ///
    /// Provides a snapshot of which slots are being used for writing,
    /// in-flight, and GPU reading at the current frame.
    #[inline]
    pub fn rotation_state(&self) -> RotationState {
        RotationState::for_frame(self.frame_count)
    }

    /// Returns the index of the slot the GPU is currently reading from
    ///
    /// This is the slot that was written 2 frames ago.
    /// Uses modular arithmetic that works correctly even for early frames.
    #[inline]
    pub fn gpu_read_index(&self) -> usize {
        // Add BUFFER_COUNT before subtracting to avoid underflow
        ((self.frame_count + BUFFER_COUNT as u64 - 2) % BUFFER_COUNT as u64) as usize
    }

    /// Returns the index of the slot that is currently in-flight
    ///
    /// This is the slot that was written 1 frame ago and may still
    /// be in the GPU command queue.
    /// Uses modular arithmetic that works correctly even for early frames.
    #[inline]
    pub fn in_flight_index(&self) -> usize {
        // Add BUFFER_COUNT before subtracting to avoid underflow
        ((self.frame_count + BUFFER_COUNT as u64 - 1) % BUFFER_COUNT as u64) as usize
    }

    /// Checks if any slot needs buffer reallocation for the given size
    ///
    /// When capacity needs to grow, ALL slots must be reallocated together
    /// to maintain consistent capacity across the rotation.
    pub fn any_slot_needs_reallocation(&self, required_size: usize) -> bool {
        self.slots.iter().any(|slot| slot.needs_reallocation(required_size))
    }

    /// Invalidates all bind groups across all slots
    ///
    /// Call this when shared resources (view uniform, config) are recreated,
    /// since all bind groups reference these shared buffers.
    pub fn invalidate_all_bind_groups(&mut self) {
        for slot in &mut self.slots {
            slot.invalidate_bind_group();
        }
    }

    /// Checks if the view uniform needs updating based on viewport/color changes
    pub fn view_uniform_changed(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        bull_color: [f32; 4],
        bear_color: [f32; 4],
        wick_color: [f32; 4],
    ) -> bool {
        self.last_viewport_width != viewport_width
            || self.last_viewport_height != viewport_height
            || self.last_bull_color != bull_color
            || self.last_bear_color != bear_color
            || self.last_wick_color != wick_color
    }

    /// Updates the cached viewport and color values
    pub fn update_view_tracking(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        bull_color: [f32; 4],
        bear_color: [f32; 4],
        wick_color: [f32; 4],
    ) {
        self.last_viewport_width = viewport_width;
        self.last_viewport_height = viewport_height;
        self.last_bull_color = bull_color;
        self.last_bear_color = bear_color;
        self.last_wick_color = wick_color;
    }

    // ==================== Allocation Management (Phase 5) ====================

    /// Returns the recommended capacity for initial pre-allocation
    ///
    /// Uses extra growth factor for initial allocation to reduce
    /// the likelihood of early reallocations.
    ///
    /// # Arguments
    /// * `required_size` - The size of initial data in bytes
    pub fn recommended_initial_capacity(&self, required_size: usize) -> usize {
        calculate_initial_capacity(required_size)
    }

    /// Returns the recommended capacity for reallocation
    ///
    /// Uses standard growth factor for subsequent reallocations.
    ///
    /// # Arguments
    /// * `required_size` - The size needed in bytes
    pub fn recommended_grown_capacity(&self, required_size: usize) -> usize {
        calculate_grown_capacity(required_size)
    }

    /// Records initial allocation statistics for all slots
    ///
    /// Call this after pre-allocating all 3 slots with the same capacity.
    ///
    /// # Arguments
    /// * `capacity_per_slot` - The capacity allocated for each slot
    pub fn record_initial_allocation(&mut self, capacity_per_slot: usize) {
        // Record for all 3 slots
        for _ in 0..BUFFER_COUNT {
            self.allocation_stats.record_initial_allocation(capacity_per_slot);
        }
    }

    /// Records a single slot reallocation
    ///
    /// Call this when a slot's buffer is reallocated.
    ///
    /// # Arguments
    /// * `old_capacity` - The previous capacity of the slot
    /// * `new_capacity` - The new capacity of the slot
    pub fn record_slot_reallocation(&mut self, old_capacity: usize, new_capacity: usize) {
        self.allocation_stats.record_reallocation(old_capacity, new_capacity);
    }

    /// Updates instance count tracking in allocation stats
    ///
    /// # Arguments
    /// * `count` - The current instance count
    pub fn track_instance_count(&mut self, count: u32) {
        self.allocation_stats.record_instance_count(count);
    }

    /// Returns the current allocation statistics
    pub fn get_allocation_stats(&self) -> &AllocationStats {
        &self.allocation_stats
    }

    /// Returns the maximum capacity across all slots
    pub fn max_slot_capacity(&self) -> usize {
        self.slots.iter().map(|s| s.buffer_capacity).max().unwrap_or(0)
    }

    /// Returns the minimum capacity across all slots
    ///
    /// Useful to detect if slots have inconsistent capacities (which shouldn't happen).
    pub fn min_slot_capacity(&self) -> usize {
        self.slots.iter().map(|s| s.buffer_capacity).min().unwrap_or(0)
    }

    /// Checks if all slots have consistent capacity
    ///
    /// All slots should have the same capacity for predictable behavior.
    pub fn capacities_consistent(&self) -> bool {
        self.max_slot_capacity() == self.min_slot_capacity()
    }

    /// Returns total memory used by instance buffers across all slots
    pub fn total_instance_buffer_memory(&self) -> usize {
        self.slots.iter().map(|s| s.buffer_capacity).sum()
    }

    /// Returns total memory used by uniform buffers across all slots
    pub fn total_uniform_buffer_memory(&self) -> usize {
        self.slots.iter().map(|s| s.uniform_buffer_capacity).sum()
    }

    /// Returns a summary of memory usage for debugging
    pub fn memory_summary(&self) -> String {
        let instance_mem = self.total_instance_buffer_memory();
        let uniform_mem = self.total_uniform_buffer_memory();
        let total = instance_mem + uniform_mem;
        let stats = &self.allocation_stats;

        format!(
            "Memory: {} total ({} instance, {} uniform) | {}",
            format_bytes(total),
            format_bytes(instance_mem),
            format_bytes(uniform_mem),
            stats.summary()
        )
    }

    // ==================== Debug Instrumentation (Phase 6) ====================

    /// Enables debug statistics tracking
    pub fn enable_debug(&mut self) {
        self.debug_stats.enable();
    }

    /// Disables debug statistics tracking
    pub fn disable_debug(&mut self) {
        self.debug_stats.disable();
    }

    /// Returns whether debug tracking is enabled
    pub fn is_debug_enabled(&self) -> bool {
        self.debug_stats.enabled
    }

    /// Returns reference to debug stats
    pub fn get_debug_stats(&self) -> &DebugStats {
        &self.debug_stats
    }

    /// Begins debug tracking for current frame
    ///
    /// Call this at the start of frame processing.
    pub fn debug_begin_frame(&mut self) {
        let rotation = self.rotation_state();
        self.debug_stats.begin_frame(
            rotation.frame,
            rotation.write_slot,
            rotation.gpu_read_slot,
        );
    }

    /// Records a fence wait for debug stats
    pub fn debug_record_fence_wait(&mut self, wait_us: u64) {
        self.debug_stats.record_fence_wait(wait_us);
    }

    /// Records a reallocation for debug stats
    pub fn debug_record_reallocation(&mut self) {
        self.debug_stats.record_reallocation();
    }

    /// Records instance count for debug stats
    pub fn debug_record_instance_count(&mut self, count: u32) {
        self.debug_stats.record_instance_count(count);
    }

    /// Ends debug tracking for current frame
    ///
    /// Call this at the end of frame processing.
    pub fn debug_end_frame(&mut self) {
        self.debug_stats.end_frame();
    }

    /// Returns a full debug report
    pub fn debug_report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== Triple Buffer Debug Report ===\n\n");

        // Rotation state
        let rotation = self.rotation_state();
        report.push_str(&format!("Current State: {}\n", rotation));

        // Memory usage
        report.push_str(&format!("{}\n\n", self.memory_summary()));

        // Debug stats
        report.push_str(&self.debug_stats.summary());
        report.push_str("\n\n");

        // Fence states
        report.push_str("Fence States:\n");
        let summary = self.fence_summary();
        for (idx, state, frame) in summary.iter() {
            report.push_str(&format!(
                "  Slot {}: {:?} (submitted: {:?})\n",
                idx, state, frame
            ));
        }

        // Health check
        report.push_str("\nHealth Check: ");
        match self.debug_stats.health_check() {
            Ok(()) => report.push_str("HEALTHY ✓\n"),
            Err(issues) => {
                report.push_str("ISSUES DETECTED:\n");
                report.push_str(&issues);
                report.push('\n');
            }
        }

        report
    }

    /// Prints debug report to stdout (debug builds only)
    #[cfg(debug_assertions)]
    pub fn print_debug_report(&self) {
        println!("{}", self.debug_report());
    }

    // ==================== Fence Management ====================

    /// Checks if the current slot is safe to write to
    ///
    /// Returns true if enough frames have passed since the slot was last used,
    /// ensuring the GPU is done with any previous data in this slot.
    pub fn current_slot_is_safe(&self) -> bool {
        self.current_slot().is_safe_for_write(self.frame_count)
    }

    /// Begins writing to the current slot
    ///
    /// Updates the fence state to indicate CPU is writing.
    /// Should be called before writing data to the buffer.
    pub fn begin_frame_write(&mut self) {
        self.current_slot_mut().begin_write();
    }

    /// Ends writing to the current slot and marks it as submitted
    ///
    /// Updates the fence state to indicate GPU work is pending.
    /// Should be called after writing data and submitting GPU commands.
    pub fn end_frame_write(&mut self) {
        let frame = self.frame_count;
        self.current_slot_mut().end_write(frame);
    }

    /// Updates fence states based on frame progression
    ///
    /// Marks slots as complete if enough frames have passed since submission.
    /// Call this at the start of each frame to update fence states.
    pub fn update_fence_states(&mut self) {
        for slot in &mut self.slots {
            if slot.fence.state == FrameSyncState::Submitted {
                if slot.fence.is_ready(self.frame_count) {
                    slot.mark_complete();
                }
            }
        }
    }

    /// Returns a summary of all fence states for debugging
    pub fn fence_summary(&self) -> [(usize, FrameSyncState, Option<u64>); BUFFER_COUNT] {
        [
            (0, self.slots[0].fence.state, self.slots[0].fence.submission_frame),
            (1, self.slots[1].fence.state, self.slots[1].fence.submission_frame),
            (2, self.slots[2].fence.state, self.slots[2].fence.submission_frame),
        ]
    }

    /// Checks if any slot has a pending fence (GPU work in progress)
    pub fn has_pending_fences(&self) -> bool {
        self.slots.iter().any(|slot| slot.fence.state == FrameSyncState::Submitted)
    }

    /// Waits (logically) for all fences to complete
    ///
    /// Marks all submitted slots as complete. Use this when you need to
    /// ensure all GPU work is done (e.g., before reallocating all buffers).
    ///
    /// Note: This is a logical wait. Actual GPU synchronization is handled
    /// by wgpu/Bevy internally.
    pub fn wait_all_fences(&mut self) {
        for slot in &mut self.slots {
            if slot.fence.state == FrameSyncState::Submitted {
                slot.mark_complete();
            }
        }
    }

    /// Resets all fence states (use after reallocating all buffers)
    pub fn reset_all_fences(&mut self) {
        for slot in &mut self.slots {
            slot.fence.reset();
        }
    }

    // ==================== Synchronization Flow ====================
    //
    // The complete per-frame workflow:
    //
    // ```text
    // ┌─────────────────────────────────────────────────────────────┐
    // │              PER-FRAME SYNCHRONIZATION FLOW                 │
    // ├─────────────────────────────────────────────────────────────┤
    // │                                                             │
    // │  1. BEGIN FRAME                                             │
    // │     └─ update_fence_states() - auto-complete ready fences   │
    // │                                                             │
    // │  2. ACQUIRE SLOT                                            │
    // │     ├─ current_index() - get slot for this frame            │
    // │     └─ wait_for_current_slot() - ensure slot is ready       │
    // │                                                             │
    // │  3. WRITE DATA                                              │
    // │     ├─ begin_frame_write() - mark slot as writing           │
    // │     ├─ [write instance data to buffer]                      │
    // │     └─ end_frame_write() - mark slot as submitted           │
    // │                                                             │
    // │  4. RENDER                                                  │
    // │     └─ [GPU reads from current slot's buffer]               │
    // │                                                             │
    // │  5. END FRAME                                               │
    // │     └─ advance_frame() - move to next slot                  │
    // │                                                             │
    // └─────────────────────────────────────────────────────────────┘
    // ```

    /// Result of attempting to acquire a buffer slot for writing
    #[must_use]
    pub fn try_acquire_slot(&mut self) -> SlotAcquisition {
        // Update fence states first
        self.update_fence_states();

        let index = self.current_index();
        let slot = &self.slots[index];

        if slot.fence.is_ready(self.frame_count) {
            SlotAcquisition::Ready { slot_index: index }
        } else {
            let frames_remaining = slot.fence.submission_frame
                .map(|sub| FRAMES_UNTIL_SAFE.saturating_sub(self.frame_count.saturating_sub(sub)))
                .unwrap_or(0);
            SlotAcquisition::Busy {
                slot_index: index,
                frames_remaining,
            }
        }
    }

    /// Waits for the current slot to be ready, returning immediately if already ready
    ///
    /// In practice with triple buffering, this should never actually wait because
    /// by the time we cycle back to a slot (3 frames later), the GPU work from
    /// that slot's last use (2+ frames ago) should be complete.
    ///
    /// Returns the slot index that is now ready for writing.
    pub fn wait_for_current_slot(&mut self) -> usize {
        self.update_fence_states();

        let index = self.current_index();

        // With triple buffering, slot should always be ready
        // But if not, force it complete (wgpu handles actual sync)
        if !self.slots[index].fence.is_ready(self.frame_count) {
            // This indicates we're CPU-bound (generating frames faster than GPU can consume)
            // In a real scenario, wgpu's internal synchronization prevents data races
            self.slots[index].mark_complete();
        }

        index
    }

    /// Executes the complete per-frame synchronization workflow
    ///
    /// This is a convenience method that combines all synchronization steps.
    /// Returns a `FrameContext` containing the slot index and references needed
    /// for the current frame's rendering.
    ///
    /// # Workflow
    /// 1. Updates fence states for all slots
    /// 2. Acquires the current slot (waits if necessary)
    /// 3. Stores the write slot for render to read later
    /// 4. Marks the slot as being written
    ///
    /// After calling this, write your data to the buffer, then call `complete_frame()`.
    pub fn begin_frame(&mut self) -> FrameContext {
        // Step 1: Update fence states
        self.update_fence_states();

        // Step 2: Acquire current slot
        let slot_index = self.wait_for_current_slot();

        // Step 3: Store which slot we're writing to for render to read later
        self.current_write_slot = slot_index;

        // Step 4: Begin writing
        self.begin_frame_write();

        FrameContext {
            frame_number: self.frame_count,
            slot_index,
            instance_count: self.instance_count,
        }
    }

    /// Completes the current frame after data has been written
    ///
    /// # Workflow
    /// 1. Marks the current slot as submitted to GPU
    /// 2. Advances to the next frame
    ///
    /// Call this after writing instance data to the buffer.
    pub fn complete_frame(&mut self) {
        // Step 1: Mark as submitted
        self.end_frame_write();

        // Step 2: Advance frame counter
        self.advance_frame();
    }

    /// Performs a complete frame cycle for cases where no data needs to be written
    ///
    /// Use this when you want to advance the frame counter but don't have new
    /// instance data to write (e.g., when nothing changed).
    pub fn skip_frame(&mut self) {
        self.update_fence_states();
        self.advance_frame();
    }
}
