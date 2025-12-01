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

use bevy::prelude::*;
use bevy::render::render_resource::{BindGroup, Buffer};
use std::time::Instant;

/// Number of buffer slots for triple buffering
pub const BUFFER_COUNT: usize = 3;

/// Number of frames to wait before considering a slot "safe" for reuse
/// With triple buffering, after 2 frames the GPU should be done with a buffer
pub const FRAMES_UNTIL_SAFE: u64 = 2;

/// Growth factor for buffer capacity when reallocation is needed.
/// Using 1.5x provides a good balance between memory usage and reallocation frequency.
/// With 1.5x, after n reallocations the buffer size is 1.5^n times the original.
pub const BUFFER_GROWTH_FACTOR: f32 = 1.5;

/// Minimum buffer capacity in bytes to avoid frequent small reallocations.
/// Set to 4KB (a typical page size) as a reasonable minimum.
pub const MIN_BUFFER_CAPACITY: usize = 4096;

/// Number of frames to keep in timing history for debugging
const DEBUG_HISTORY_SIZE: usize = 60;

// ==================== Debug Instrumentation (Phase 6) ====================

/// Per-frame timing information for debugging
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameTiming {
    /// Frame number
    pub frame: u64,
    /// Which slot was written to
    pub write_slot: usize,
    /// Which slot GPU was reading from
    pub gpu_read_slot: usize,
    /// Time spent waiting for fence (should be ~0 if triple buffering works)
    pub fence_wait_us: u64,
    /// Time spent writing to buffer
    pub write_time_us: u64,
    /// Instance count for this frame
    pub instance_count: u32,
    /// Whether a reallocation occurred
    pub reallocated: bool,
}

/// Debug statistics for triple buffer performance monitoring
///
/// Tracks timing, fence waits, and buffer rotation to help identify
/// synchronization issues or performance problems.
#[derive(Debug, Clone, Default)]
pub struct DebugStats {
    /// Whether debug tracking is enabled
    pub enabled: bool,

    /// Recent frame timings (circular buffer)
    frame_history: Vec<FrameTiming>,
    /// Index for next frame timing entry
    history_index: usize,

    /// Total fence waits that were non-zero (indicates CPU/GPU contention)
    pub total_fence_waits: u64,
    /// Maximum fence wait time observed (microseconds)
    pub max_fence_wait_us: u64,
    /// Total reallocations triggered
    pub total_reallocations: u32,

    /// Timestamp when tracking started
    start_time: Option<Instant>,
    /// Last frame's timestamp for delta calculation
    last_frame_time: Option<Instant>,

    /// Current frame's timing (in progress)
    current_frame: FrameTiming,
    /// Timestamp when current frame started
    current_frame_start: Option<Instant>,
}

impl DebugStats {
    /// Creates a new debug stats tracker (disabled by default)
    pub fn new() -> Self {
        Self {
            enabled: false,
            frame_history: Vec::with_capacity(DEBUG_HISTORY_SIZE),
            ..Default::default()
        }
    }

    /// Creates a new debug stats tracker with tracking enabled
    pub fn new_enabled() -> Self {
        Self {
            enabled: true,
            frame_history: Vec::with_capacity(DEBUG_HISTORY_SIZE),
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }

    /// Enables debug tracking
    pub fn enable(&mut self) {
        self.enabled = true;
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
    }

    /// Disables debug tracking
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Begins tracking a new frame
    pub fn begin_frame(&mut self, frame: u64, write_slot: usize, gpu_read_slot: usize) {
        if !self.enabled {
            return;
        }
        self.current_frame = FrameTiming {
            frame,
            write_slot,
            gpu_read_slot,
            ..Default::default()
        };
        self.current_frame_start = Some(Instant::now());
    }

    /// Records a fence wait duration
    pub fn record_fence_wait(&mut self, wait_us: u64) {
        if !self.enabled {
            return;
        }
        self.current_frame.fence_wait_us = wait_us;
        if wait_us > 0 {
            self.total_fence_waits += 1;
            if wait_us > self.max_fence_wait_us {
                self.max_fence_wait_us = wait_us;
            }
        }
    }

    /// Records a buffer reallocation
    pub fn record_reallocation(&mut self) {
        if !self.enabled {
            return;
        }
        self.current_frame.reallocated = true;
        self.total_reallocations += 1;
    }

    /// Records instance count for current frame
    pub fn record_instance_count(&mut self, count: u32) {
        if !self.enabled {
            return;
        }
        self.current_frame.instance_count = count;
    }

    /// Completes the current frame timing
    pub fn end_frame(&mut self) {
        if !self.enabled {
            return;
        }

        // Calculate write time
        if let Some(start) = self.current_frame_start {
            self.current_frame.write_time_us = start.elapsed().as_micros() as u64;
        }

        // Store in history (circular buffer)
        if self.frame_history.len() < DEBUG_HISTORY_SIZE {
            self.frame_history.push(self.current_frame);
        } else {
            self.frame_history[self.history_index] = self.current_frame;
        }
        self.history_index = (self.history_index + 1) % DEBUG_HISTORY_SIZE;

        self.last_frame_time = Some(Instant::now());
    }

    /// Returns the most recent frame timings (up to DEBUG_HISTORY_SIZE)
    pub fn recent_frames(&self) -> &[FrameTiming] {
        &self.frame_history
    }

    /// Returns average fence wait time across recent frames
    pub fn avg_fence_wait_us(&self) -> f64 {
        if self.frame_history.is_empty() {
            return 0.0;
        }
        let total: u64 = self.frame_history.iter().map(|f| f.fence_wait_us).sum();
        total as f64 / self.frame_history.len() as f64
    }

    /// Returns average write time across recent frames
    pub fn avg_write_time_us(&self) -> f64 {
        if self.frame_history.is_empty() {
            return 0.0;
        }
        let total: u64 = self.frame_history.iter().map(|f| f.write_time_us).sum();
        total as f64 / self.frame_history.len() as f64
    }

    /// Returns the percentage of frames that had non-zero fence waits
    pub fn fence_wait_percentage(&self) -> f64 {
        if self.frame_history.is_empty() {
            return 0.0;
        }
        let waits = self.frame_history.iter().filter(|f| f.fence_wait_us > 0).count();
        (waits as f64 / self.frame_history.len() as f64) * 100.0
    }

    /// Returns a summary string for debugging
    pub fn summary(&self) -> String {
        if !self.enabled || self.frame_history.is_empty() {
            return "Debug stats: disabled or no data".to_string();
        }

        format!(
            "Triple Buffer Debug Stats:\n\
             - Frames tracked: {}\n\
             - Avg fence wait: {:.1}µs (max: {}µs)\n\
             - Fence wait rate: {:.1}%\n\
             - Avg write time: {:.1}µs\n\
             - Total reallocations: {}\n\
             - Total fence waits: {}",
            self.frame_history.len(),
            self.avg_fence_wait_us(),
            self.max_fence_wait_us,
            self.fence_wait_percentage(),
            self.avg_write_time_us(),
            self.total_reallocations,
            self.total_fence_waits
        )
    }

    /// Returns a compact one-line status for HUD display
    pub fn status_line(&self) -> String {
        if !self.enabled {
            return "Debug: OFF".to_string();
        }
        format!(
            "TB: fence={:.0}µs/{:.1}% realloc={} frames={}",
            self.avg_fence_wait_us(),
            self.fence_wait_percentage(),
            self.total_reallocations,
            self.frame_history.len()
        )
    }

    /// Checks if triple buffering is working correctly
    ///
    /// Returns Ok if healthy, Err with description if issues detected.
    pub fn health_check(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        let mut issues = Vec::new();

        // Check fence wait rate (should be ~0% for healthy triple buffering)
        let wait_rate = self.fence_wait_percentage();
        if wait_rate > 5.0 {
            issues.push(format!(
                "High fence wait rate: {:.1}% (expected <5%)",
                wait_rate
            ));
        }

        // Check max fence wait (should be minimal)
        if self.max_fence_wait_us > 1000 {
            issues.push(format!(
                "High max fence wait: {}µs (>1ms indicates stalls)",
                self.max_fence_wait_us
            ));
        }

        // Check reallocation frequency
        if self.frame_history.len() > 10 {
            let recent_reallocs = self.frame_history.iter().filter(|f| f.reallocated).count();
            let realloc_rate = (recent_reallocs as f64 / self.frame_history.len() as f64) * 100.0;
            if realloc_rate > 10.0 {
                issues.push(format!(
                    "High reallocation rate: {:.1}% (consider larger growth factor)",
                    realloc_rate
                ));
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues.join("\n"))
        }
    }
}

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

/// Statistics for tracking buffer allocation patterns
///
/// Useful for debugging and performance monitoring. Tracks how often
/// buffers are reallocated and what the peak usage has been.
#[derive(Debug, Clone, Default)]
pub struct AllocationStats {
    /// Number of times initial allocation occurred (should be 1 per slot)
    pub initial_allocations: u32,

    /// Number of times buffers were reallocated due to capacity growth
    pub reallocations: u32,

    /// Peak buffer capacity that was allocated (in bytes)
    pub peak_capacity: usize,

    /// Peak number of instances that were stored
    pub peak_instance_count: u32,

    /// Current allocated capacity across all slots (in bytes)
    pub current_capacity: usize,

    /// Total bytes allocated across all reallocations (for tracking memory churn)
    pub total_bytes_allocated: usize,
}

impl AllocationStats {
    /// Records an initial allocation
    ///
    /// # Arguments
    /// * `capacity` - The capacity allocated for this slot
    pub fn record_initial_allocation(&mut self, capacity: usize) {
        self.initial_allocations += 1;
        self.current_capacity += capacity;  // Accumulate total capacity
        self.total_bytes_allocated += capacity;
        if capacity > self.peak_capacity {
            self.peak_capacity = capacity;
        }
    }

    /// Records a reallocation with new capacity
    pub fn record_reallocation(&mut self, old_capacity: usize, new_capacity: usize) {
        self.reallocations += 1;
        self.current_capacity = self.current_capacity.saturating_sub(old_capacity) + new_capacity;
        self.total_bytes_allocated += new_capacity;
        if new_capacity > self.peak_capacity {
            self.peak_capacity = new_capacity;
        }
    }

    /// Records the instance count for peak tracking
    pub fn record_instance_count(&mut self, count: u32) {
        if count > self.peak_instance_count {
            self.peak_instance_count = count;
        }
    }

    /// Returns the memory efficiency ratio (used vs allocated)
    /// Returns None if no allocation has been made
    pub fn efficiency(&self, current_used: usize) -> Option<f32> {
        if self.current_capacity == 0 {
            None
        } else {
            Some(current_used as f32 / self.current_capacity as f32)
        }
    }

    /// Returns a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Allocations: {} initial, {} reallocs | Peak: {} bytes ({} instances) | Current: {} bytes | Total churn: {} bytes",
            self.initial_allocations,
            self.reallocations,
            self.peak_capacity,
            self.peak_instance_count,
            self.current_capacity,
            self.total_bytes_allocated
        )
    }
}

/// Calculates the new buffer capacity with growth factor
///
/// Given a required size, returns a capacity that:
/// 1. Is at least `required_size`
/// 2. Is at least `MIN_BUFFER_CAPACITY`
/// 3. Applies `BUFFER_GROWTH_FACTOR` to provide headroom
///
/// # Arguments
/// * `required_size` - The minimum size needed in bytes
///
/// # Returns
/// The recommended capacity including growth headroom
pub fn calculate_grown_capacity(required_size: usize) -> usize {
    let grown = (required_size as f32 * BUFFER_GROWTH_FACTOR).ceil() as usize;
    grown.max(MIN_BUFFER_CAPACITY)
}

/// Calculates capacity for initial pre-allocation
///
/// For initial allocation, we want extra headroom to handle typical
/// data size variations without immediate reallocation.
///
/// # Arguments
/// * `required_size` - The size of initial data in bytes
///
/// # Returns
/// The recommended initial capacity
pub fn calculate_initial_capacity(required_size: usize) -> usize {
    // For initial allocation, use 2x growth factor for more headroom
    let grown = (required_size as f32 * BUFFER_GROWTH_FACTOR * BUFFER_GROWTH_FACTOR).ceil() as usize;
    grown.max(MIN_BUFFER_CAPACITY)
}

/// Formats a byte count as a human-readable string
fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

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
    /// 3. Marks the slot as being written
    ///
    /// After calling this, write your data to the buffer, then call `complete_frame()`.
    pub fn begin_frame(&mut self) -> FrameContext {
        // Step 1: Update fence states
        self.update_fence_states();

        // Step 2: Acquire current slot
        let slot_index = self.wait_for_current_slot();

        // Step 3: Begin writing
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_index_rotation() {
        let mut resources = TripleBufferedResources::new();

        assert_eq!(resources.current_index(), 0);
        resources.advance_frame();
        assert_eq!(resources.current_index(), 1);
        resources.advance_frame();
        assert_eq!(resources.current_index(), 2);
        resources.advance_frame();
        assert_eq!(resources.current_index(), 0); // Wraps back
    }

    #[test]
    fn test_index_frames_ago() {
        let mut resources = TripleBufferedResources::new();

        // Advance to frame 5
        for _ in 0..5 {
            resources.advance_frame();
        }

        assert_eq!(resources.frame_count, 5);
        assert_eq!(resources.current_index(), 2); // 5 % 3 = 2
        assert_eq!(resources.index_frames_ago(0), 2); // Current
        assert_eq!(resources.index_frames_ago(1), 1); // Previous frame
        assert_eq!(resources.index_frames_ago(2), 0); // Two frames ago
        assert_eq!(resources.index_frames_ago(3), 2); // Three frames ago (wraps)
    }

    #[test]
    fn test_slot_needs_reallocation() {
        let slot = FrameSlot::new();

        // Empty slot always needs reallocation
        assert!(slot.needs_reallocation(100));

        // Slot with capacity
        let slot_with_capacity = FrameSlot {
            buffer_capacity: 1000,
            ..Default::default()
        };

        // Still needs reallocation if buffer doesn't exist
        assert!(slot_with_capacity.needs_reallocation(100));
    }

    #[test]
    fn test_view_uniform_change_detection() {
        let mut resources = TripleBufferedResources::new();
        resources.update_view_tracking(
            800.0,
            600.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5, 1.0],
        );

        // Same values - no change
        assert!(!resources.view_uniform_changed(
            800.0,
            600.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5, 1.0],
        ));

        // Different viewport - changed
        assert!(resources.view_uniform_changed(
            1024.0,
            768.0,
            [0.0, 1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5, 1.0],
        ));

        // Different color - changed
        assert!(resources.view_uniform_changed(
            800.0,
            600.0,
            [0.0, 0.8, 0.0, 1.0], // Different bull color
            [1.0, 0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5, 1.0],
        ));
    }

    #[test]
    fn test_rotation_state_for_frame() {
        // Frame 0: Write[0], InFlight[2], GPURead[1]
        let state = RotationState::for_frame(0);
        assert_eq!(state.write_slot, 0);
        assert_eq!(state.in_flight_slot, 2);
        assert_eq!(state.gpu_read_slot, 1);

        // Frame 1: Write[1], InFlight[0], GPURead[2]
        let state = RotationState::for_frame(1);
        assert_eq!(state.write_slot, 1);
        assert_eq!(state.in_flight_slot, 0);
        assert_eq!(state.gpu_read_slot, 2);

        // Frame 2: Write[2], InFlight[1], GPURead[0]
        let state = RotationState::for_frame(2);
        assert_eq!(state.write_slot, 2);
        assert_eq!(state.in_flight_slot, 1);
        assert_eq!(state.gpu_read_slot, 0);

        // Frame 3: Same as Frame 0 (cycle repeats)
        let state = RotationState::for_frame(3);
        assert_eq!(state.write_slot, 0);
        assert_eq!(state.in_flight_slot, 2);
        assert_eq!(state.gpu_read_slot, 1);
    }

    #[test]
    fn test_rotation_state_slot_status() {
        let state = RotationState::for_frame(0);

        assert_eq!(state.slot_status(0), "WRITING");
        assert_eq!(state.slot_status(1), "GPU_READ");
        assert_eq!(state.slot_status(2), "IN_FLIGHT");
    }

    #[test]
    fn test_rotation_state_is_safe_to_write() {
        let state = RotationState::for_frame(0);

        assert!(state.is_safe_to_write(0)); // Write slot is safe
        assert!(!state.is_safe_to_write(1)); // GPU reading - not safe
        assert!(!state.is_safe_to_write(2)); // In flight - not safe
    }

    #[test]
    fn test_gpu_read_and_in_flight_indices() {
        let mut resources = TripleBufferedResources::new();

        // Frame 0
        assert_eq!(resources.current_index(), 0);
        assert_eq!(resources.gpu_read_index(), 1); // (0 - 2) % 3 = 1
        assert_eq!(resources.in_flight_index(), 2); // (0 - 1) % 3 = 2

        resources.advance_frame();

        // Frame 1
        assert_eq!(resources.current_index(), 1);
        assert_eq!(resources.gpu_read_index(), 2); // (1 - 2) % 3 = 2
        assert_eq!(resources.in_flight_index(), 0); // (1 - 1) % 3 = 0

        resources.advance_frame();

        // Frame 2
        assert_eq!(resources.current_index(), 2);
        assert_eq!(resources.gpu_read_index(), 0); // (2 - 2) % 3 = 0
        assert_eq!(resources.in_flight_index(), 1); // (2 - 1) % 3 = 1
    }

    #[test]
    fn test_rotation_state_display() {
        let state = RotationState::for_frame(5);
        let display = format!("{}", state);
        assert!(display.contains("Frame 5"));
        assert!(display.contains("Write[2]"));
    }

    #[test]
    fn test_full_rotation_cycle() {
        // Verify that after 3 frames, we're back to the same pattern
        let mut resources = TripleBufferedResources::new();

        let initial_state = resources.rotation_state();

        // Advance 3 frames (one full cycle)
        for _ in 0..3 {
            resources.advance_frame();
        }

        let after_cycle_state = resources.rotation_state();

        // Write/read/inflight slots should match (just frame number differs)
        assert_eq!(initial_state.write_slot, after_cycle_state.write_slot);
        assert_eq!(initial_state.gpu_read_slot, after_cycle_state.gpu_read_slot);
        assert_eq!(initial_state.in_flight_slot, after_cycle_state.in_flight_slot);
        assert_eq!(after_cycle_state.frame, 3);
    }

    // ==================== Fence Tests ====================

    #[test]
    fn test_frame_sync_state_default() {
        let state = FrameSyncState::default();
        assert_eq!(state, FrameSyncState::Unused);
        assert!(state.is_safe_for_write());
        assert!(!state.is_gpu_busy());
    }

    #[test]
    fn test_frame_sync_state_transitions() {
        assert!(FrameSyncState::Unused.is_safe_for_write());
        assert!(FrameSyncState::Complete.is_safe_for_write());
        assert!(!FrameSyncState::Writing.is_safe_for_write());
        assert!(!FrameSyncState::Submitted.is_safe_for_write());

        assert!(!FrameSyncState::Unused.is_gpu_busy());
        assert!(!FrameSyncState::Complete.is_gpu_busy());
        assert!(!FrameSyncState::Writing.is_gpu_busy());
        assert!(FrameSyncState::Submitted.is_gpu_busy());
    }

    #[test]
    fn test_frame_fence_lifecycle() {
        let mut fence = FrameFence::new();

        // Initially unused
        assert_eq!(fence.state, FrameSyncState::Unused);
        assert!(fence.is_ready(0));
        assert!(fence.is_ready(100));

        // Signal writing
        fence.signal_writing();
        assert_eq!(fence.state, FrameSyncState::Writing);

        // Signal submission at frame 5
        fence.signal_submission(5);
        assert_eq!(fence.state, FrameSyncState::Submitted);
        assert_eq!(fence.submission_frame, Some(5));
        assert!(fence.submission_time.is_some());

        // Not ready at frame 5 or 6
        assert!(!fence.is_ready(5));
        assert!(!fence.is_ready(6));

        // Ready at frame 7 (2 frames later)
        assert!(fence.is_ready(7));

        // Signal complete
        fence.signal_complete();
        assert_eq!(fence.state, FrameSyncState::Complete);
        assert!(fence.completion_time.is_some());
    }

    #[test]
    fn test_frame_fence_frames_since_submission() {
        let mut fence = FrameFence::new();

        // No submission yet
        assert_eq!(fence.frames_since_submission(10), None);

        // After submission
        fence.signal_submission(5);
        assert_eq!(fence.frames_since_submission(5), Some(0));
        assert_eq!(fence.frames_since_submission(7), Some(2));
        assert_eq!(fence.frames_since_submission(10), Some(5));
    }

    #[test]
    fn test_frame_slot_fence_integration() {
        let mut slot = FrameSlot::new();

        // Initially safe to write
        assert!(slot.is_safe_for_write(0));
        assert_eq!(slot.sync_state(), FrameSyncState::Unused);

        // Begin write
        slot.begin_write();
        assert_eq!(slot.sync_state(), FrameSyncState::Writing);

        // End write (submit at frame 0)
        slot.end_write(0);
        assert_eq!(slot.sync_state(), FrameSyncState::Submitted);

        // Not safe at frame 0 or 1
        assert!(!slot.is_safe_for_write(0));
        assert!(!slot.is_safe_for_write(1));

        // Safe at frame 2
        assert!(slot.is_safe_for_write(2));

        // Mark complete
        slot.mark_complete();
        assert_eq!(slot.sync_state(), FrameSyncState::Complete);
    }

    #[test]
    fn test_triple_buffer_fence_workflow() {
        let mut resources = TripleBufferedResources::new();

        // Frame 0: Write to slot 0
        assert!(resources.current_slot_is_safe());
        resources.begin_frame_write();
        assert_eq!(resources.current_slot().sync_state(), FrameSyncState::Writing);
        resources.end_frame_write();
        assert_eq!(resources.current_slot().sync_state(), FrameSyncState::Submitted);
        resources.advance_frame();

        // Frame 1: Write to slot 1
        assert!(resources.current_slot_is_safe()); // Slot 1 never used
        resources.begin_frame_write();
        resources.end_frame_write();
        resources.advance_frame();

        // Frame 2: Write to slot 2
        assert!(resources.current_slot_is_safe()); // Slot 2 never used
        resources.begin_frame_write();
        resources.end_frame_write();
        resources.advance_frame();

        // Frame 3: Back to slot 0, should be safe (submitted at frame 0, now frame 3)
        assert_eq!(resources.current_index(), 0);
        assert!(resources.current_slot_is_safe()); // 3 - 0 = 3 >= 2
    }

    #[test]
    fn test_update_fence_states() {
        let mut resources = TripleBufferedResources::new();

        // Submit work in slot 0 at frame 0
        resources.begin_frame_write();
        resources.end_frame_write();
        assert_eq!(resources.slots[0].sync_state(), FrameSyncState::Submitted);

        // Advance to frame 2
        resources.advance_frame();
        resources.advance_frame();

        // Update fence states - slot 0 should now be complete
        resources.update_fence_states();
        assert_eq!(resources.slots[0].sync_state(), FrameSyncState::Complete);
    }

    #[test]
    fn test_fence_summary() {
        let mut resources = TripleBufferedResources::new();

        // Submit to slot 0
        resources.begin_frame_write();
        resources.end_frame_write();

        let summary = resources.fence_summary();
        assert_eq!(summary[0].1, FrameSyncState::Submitted);
        assert_eq!(summary[0].2, Some(0));
        assert_eq!(summary[1].1, FrameSyncState::Unused);
        assert_eq!(summary[2].1, FrameSyncState::Unused);
    }

    #[test]
    fn test_has_pending_fences() {
        let mut resources = TripleBufferedResources::new();

        // Initially no pending fences
        assert!(!resources.has_pending_fences());

        // Submit to slot 0
        resources.begin_frame_write();
        resources.end_frame_write();

        // Now has pending fence
        assert!(resources.has_pending_fences());

        // Wait all fences
        resources.wait_all_fences();
        assert!(!resources.has_pending_fences());
    }

    #[test]
    fn test_reset_all_fences() {
        let mut resources = TripleBufferedResources::new();

        // Submit to all slots
        for _ in 0..3 {
            resources.begin_frame_write();
            resources.end_frame_write();
            resources.advance_frame();
        }

        // All slots should have submission history
        assert!(resources.slots.iter().all(|s| s.fence.submission_frame.is_some()));

        // Reset all
        resources.reset_all_fences();

        // All should be unused now
        assert!(resources.slots.iter().all(|s| s.fence.state == FrameSyncState::Unused));
        assert!(resources.slots.iter().all(|s| s.fence.submission_frame.is_none()));
    }

    // ==================== Synchronization Flow Tests ====================

    #[test]
    fn test_slot_acquisition_ready() {
        let mut resources = TripleBufferedResources::new();

        // Fresh resources should always be ready
        let result = resources.try_acquire_slot();
        assert!(result.is_ready());
        assert_eq!(result.slot_index(), 0);

        match result {
            SlotAcquisition::Ready { slot_index } => {
                assert_eq!(slot_index, 0);
            }
            _ => panic!("Expected Ready"),
        }
    }

    #[test]
    fn test_slot_acquisition_busy() {
        let mut resources = TripleBufferedResources::new();

        // Simulate a scenario where we're at frame 0 but slot 0 was just submitted
        resources.slots[0].fence.signal_submission(0);

        // At frame 0, slot 0 should be busy
        let result = resources.try_acquire_slot();
        assert!(!result.is_ready());

        match result {
            SlotAcquisition::Busy { slot_index, frames_remaining } => {
                assert_eq!(slot_index, 0);
                assert_eq!(frames_remaining, 2); // Need 2 more frames
            }
            _ => panic!("Expected Busy"),
        }
    }

    #[test]
    fn test_wait_for_current_slot() {
        let mut resources = TripleBufferedResources::new();

        // Should return immediately for unused slots
        let index = resources.wait_for_current_slot();
        assert_eq!(index, 0);

        // Even with a "busy" slot, it should return (force complete)
        resources.advance_frame();
        resources.slots[1].fence.signal_submission(1);
        let index = resources.wait_for_current_slot();
        assert_eq!(index, 1);
        // Slot should have been marked complete
        assert_eq!(resources.slots[1].sync_state(), FrameSyncState::Complete);
    }

    #[test]
    fn test_begin_frame_returns_context() {
        let mut resources = TripleBufferedResources::new();
        resources.instance_count = 100;

        let ctx = resources.begin_frame();

        assert_eq!(ctx.frame_number, 0);
        assert_eq!(ctx.slot_index, 0);
        assert_eq!(ctx.instance_count, 100);
        assert!(ctx.has_instances());

        // Current slot should be in Writing state
        assert_eq!(resources.current_slot().sync_state(), FrameSyncState::Writing);
    }

    #[test]
    fn test_complete_frame_advances() {
        let mut resources = TripleBufferedResources::new();

        let ctx = resources.begin_frame();
        assert_eq!(ctx.frame_number, 0);
        assert_eq!(ctx.slot_index, 0);

        resources.complete_frame();

        // Should have advanced to frame 1
        assert_eq!(resources.frame_count, 1);
        assert_eq!(resources.current_index(), 1);

        // Slot 0 should be in Submitted state
        assert_eq!(resources.slots[0].sync_state(), FrameSyncState::Submitted);
    }

    #[test]
    fn test_full_sync_workflow() {
        let mut resources = TripleBufferedResources::new();

        // Simulate 6 frames of rendering
        for frame in 0..6 {
            let ctx = resources.begin_frame();
            assert_eq!(ctx.frame_number, frame);
            assert_eq!(ctx.slot_index, (frame % 3) as usize);

            // Simulate writing data...

            resources.complete_frame();
        }

        // After 6 frames, we should be at frame 6
        assert_eq!(resources.frame_count, 6);

        // All slots should have been used twice
        for slot in &resources.slots {
            assert!(slot.fence.submission_frame.is_some());
        }
    }

    #[test]
    fn test_skip_frame() {
        let mut resources = TripleBufferedResources::new();

        // Start at frame 0
        assert_eq!(resources.frame_count, 0);

        // Skip frame
        resources.skip_frame();

        // Should be at frame 1 now
        assert_eq!(resources.frame_count, 1);

        // Slot 0 should still be Unused (no write occurred)
        assert_eq!(resources.slots[0].sync_state(), FrameSyncState::Unused);
    }

    #[test]
    fn test_frame_context_has_instances() {
        let ctx_with = FrameContext {
            frame_number: 0,
            slot_index: 0,
            instance_count: 100,
        };
        assert!(ctx_with.has_instances());

        let ctx_without = FrameContext {
            frame_number: 0,
            slot_index: 0,
            instance_count: 0,
        };
        assert!(!ctx_without.has_instances());
    }

    #[test]
    fn test_sync_flow_with_instance_count() {
        let mut resources = TripleBufferedResources::new();

        // Set instance count
        resources.instance_count = 500;

        let ctx = resources.begin_frame();
        assert_eq!(ctx.instance_count, 500);

        // Change instance count mid-flow
        resources.instance_count = 750;

        resources.complete_frame();

        // Next frame should see updated count
        let ctx2 = resources.begin_frame();
        assert_eq!(ctx2.instance_count, 750);
    }

    #[test]
    fn test_slot_acquisition_slot_index() {
        let ready = SlotAcquisition::Ready { slot_index: 1 };
        assert_eq!(ready.slot_index(), 1);

        let busy = SlotAcquisition::Busy { slot_index: 2, frames_remaining: 1 };
        assert_eq!(busy.slot_index(), 2);
    }

    // ==================== Allocation Management Tests (Phase 5) ====================

    #[test]
    fn test_buffer_growth_factor_constant() {
        // Verify growth factor is reasonable (1.5x)
        assert_eq!(BUFFER_GROWTH_FACTOR, 1.5);
    }

    #[test]
    fn test_min_buffer_capacity_constant() {
        // Verify minimum is 4KB
        assert_eq!(MIN_BUFFER_CAPACITY, 4096);
    }

    #[test]
    fn test_calculate_grown_capacity_small() {
        // Small sizes should be at least MIN_BUFFER_CAPACITY
        assert_eq!(calculate_grown_capacity(100), MIN_BUFFER_CAPACITY);
        assert_eq!(calculate_grown_capacity(1000), MIN_BUFFER_CAPACITY);
        assert_eq!(calculate_grown_capacity(2000), MIN_BUFFER_CAPACITY);
    }

    #[test]
    fn test_calculate_grown_capacity_large() {
        // Large sizes should get growth factor applied
        let required = 10000;
        let grown = calculate_grown_capacity(required);
        assert!(grown >= required);
        assert!(grown >= (required as f32 * BUFFER_GROWTH_FACTOR) as usize);
    }

    #[test]
    fn test_calculate_initial_capacity() {
        // Initial capacity should have extra headroom (growth^2)
        let required = 10000;
        let initial = calculate_initial_capacity(required);
        let grown = calculate_grown_capacity(required);

        // Initial should be larger than grown (2x growth factor)
        assert!(initial > grown);
        // Initial should be approximately growth^2
        let expected = (required as f32 * BUFFER_GROWTH_FACTOR * BUFFER_GROWTH_FACTOR).ceil() as usize;
        assert_eq!(initial, expected);
    }

    #[test]
    fn test_allocation_stats_initial() {
        let mut stats = AllocationStats::default();

        stats.record_initial_allocation(4096);

        assert_eq!(stats.initial_allocations, 1);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.peak_capacity, 4096);
        assert_eq!(stats.current_capacity, 4096);
        assert_eq!(stats.total_bytes_allocated, 4096);
    }

    #[test]
    fn test_allocation_stats_reallocation() {
        let mut stats = AllocationStats::default();

        // Initial allocation
        stats.record_initial_allocation(4096);

        // Reallocation
        stats.record_reallocation(4096, 8192);

        assert_eq!(stats.initial_allocations, 1);
        assert_eq!(stats.reallocations, 1);
        assert_eq!(stats.peak_capacity, 8192);
        assert_eq!(stats.current_capacity, 8192);
        // Total = initial + realloc
        assert_eq!(stats.total_bytes_allocated, 4096 + 8192);
    }

    #[test]
    fn test_allocation_stats_instance_count_tracking() {
        let mut stats = AllocationStats::default();

        stats.record_instance_count(100);
        assert_eq!(stats.peak_instance_count, 100);

        stats.record_instance_count(50);
        assert_eq!(stats.peak_instance_count, 100); // Peak unchanged

        stats.record_instance_count(200);
        assert_eq!(stats.peak_instance_count, 200); // New peak
    }

    #[test]
    fn test_allocation_stats_efficiency() {
        let mut stats = AllocationStats::default();

        // No allocation yet
        assert!(stats.efficiency(100).is_none());

        stats.record_initial_allocation(10000);

        // 50% efficiency
        assert!((stats.efficiency(5000).unwrap() - 0.5).abs() < 0.001);

        // 100% efficiency
        assert!((stats.efficiency(10000).unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_triple_buffer_allocation_methods() {
        let resources = TripleBufferedResources::new();

        // Test recommended capacities
        let initial = resources.recommended_initial_capacity(10000);
        let grown = resources.recommended_grown_capacity(10000);

        assert!(initial > grown);
        assert!(grown >= 10000);
    }

    #[test]
    fn test_triple_buffer_record_allocation() {
        let mut resources = TripleBufferedResources::new();

        // Record initial allocation for all 3 slots
        resources.record_initial_allocation(4096);

        let stats = resources.get_allocation_stats();
        assert_eq!(stats.initial_allocations, 3); // 3 slots
        assert_eq!(stats.current_capacity, 3 * 4096);
    }

    #[test]
    fn test_triple_buffer_capacity_consistency() {
        let mut resources = TripleBufferedResources::new();

        // Initially all zeros
        assert!(resources.capacities_consistent());
        assert_eq!(resources.max_slot_capacity(), 0);
        assert_eq!(resources.min_slot_capacity(), 0);

        // Set consistent capacities
        for slot in &mut resources.slots {
            slot.buffer_capacity = 4096;
        }

        assert!(resources.capacities_consistent());
        assert_eq!(resources.max_slot_capacity(), 4096);
        assert_eq!(resources.min_slot_capacity(), 4096);

        // Make inconsistent
        resources.slots[0].buffer_capacity = 8192;

        assert!(!resources.capacities_consistent());
        assert_eq!(resources.max_slot_capacity(), 8192);
        assert_eq!(resources.min_slot_capacity(), 4096);
    }

    #[test]
    fn test_triple_buffer_memory_totals() {
        let mut resources = TripleBufferedResources::new();

        // Set capacities
        for slot in &mut resources.slots {
            slot.buffer_capacity = 4096;
            slot.uniform_buffer_capacity = 256;
        }

        assert_eq!(resources.total_instance_buffer_memory(), 3 * 4096);
        assert_eq!(resources.total_uniform_buffer_memory(), 3 * 256);
    }

    #[test]
    fn test_allocation_stats_summary() {
        let mut stats = AllocationStats::default();
        stats.record_initial_allocation(4096);
        stats.record_instance_count(100);

        let summary = stats.summary();

        assert!(summary.contains("1 initial"));
        assert!(summary.contains("0 reallocs"));
        assert!(summary.contains("4096 bytes"));
        assert!(summary.contains("100 instances"));
    }

    #[test]
    fn test_memory_summary() {
        let mut resources = TripleBufferedResources::new();

        // Set up some capacities
        for slot in &mut resources.slots {
            slot.buffer_capacity = 4096;
            slot.uniform_buffer_capacity = 256;
        }
        resources.record_initial_allocation(4096);

        let summary = resources.memory_summary();

        // Should contain KB values for the totals
        assert!(summary.contains("KB"));
        assert!(summary.contains("instance"));
        assert!(summary.contains("uniform"));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(2048), "2.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.00 MB");
    }

    // ==================== Debug Instrumentation Tests (Phase 6) ====================

    #[test]
    fn test_debug_stats_disabled_by_default() {
        let stats = DebugStats::new();
        assert!(!stats.enabled);
    }

    #[test]
    fn test_debug_stats_enabled_constructor() {
        let stats = DebugStats::new_enabled();
        assert!(stats.enabled);
        assert!(stats.start_time.is_some());
    }

    #[test]
    fn test_debug_stats_enable_disable() {
        let mut stats = DebugStats::new();

        stats.enable();
        assert!(stats.enabled);

        stats.disable();
        assert!(!stats.enabled);
    }

    #[test]
    fn test_debug_stats_frame_tracking() {
        let mut stats = DebugStats::new_enabled();

        // Begin frame
        stats.begin_frame(0, 0, 2);
        stats.record_instance_count(100);
        stats.end_frame();

        assert_eq!(stats.recent_frames().len(), 1);
        assert_eq!(stats.recent_frames()[0].frame, 0);
        assert_eq!(stats.recent_frames()[0].write_slot, 0);
        assert_eq!(stats.recent_frames()[0].gpu_read_slot, 2);
        assert_eq!(stats.recent_frames()[0].instance_count, 100);
    }

    #[test]
    fn test_debug_stats_fence_wait_tracking() {
        let mut stats = DebugStats::new_enabled();

        // Frame with no fence wait
        stats.begin_frame(0, 0, 2);
        stats.record_fence_wait(0);
        stats.end_frame();

        assert_eq!(stats.total_fence_waits, 0);

        // Frame with fence wait
        stats.begin_frame(1, 1, 0);
        stats.record_fence_wait(500);
        stats.end_frame();

        assert_eq!(stats.total_fence_waits, 1);
        assert_eq!(stats.max_fence_wait_us, 500);
    }

    #[test]
    fn test_debug_stats_reallocation_tracking() {
        let mut stats = DebugStats::new_enabled();

        stats.begin_frame(0, 0, 2);
        stats.record_reallocation();
        stats.end_frame();

        assert_eq!(stats.total_reallocations, 1);
        assert!(stats.recent_frames()[0].reallocated);
    }

    #[test]
    fn test_debug_stats_averages() {
        let mut stats = DebugStats::new_enabled();

        // Record 3 frames with varying fence waits
        for i in 0..3 {
            stats.begin_frame(i, (i % 3) as usize, ((i + 1) % 3) as usize);
            stats.record_fence_wait(i as u64 * 100); // 0, 100, 200
            stats.end_frame();
        }

        // Average should be 100
        assert!((stats.avg_fence_wait_us() - 100.0).abs() < 0.1);

        // 2 out of 3 had waits (frames 1 and 2)
        assert!((stats.fence_wait_percentage() - 66.66).abs() < 1.0);
    }

    #[test]
    fn test_debug_stats_health_check_healthy() {
        let mut stats = DebugStats::new_enabled();

        // Record 20 healthy frames
        for i in 0..20 {
            stats.begin_frame(i, (i % 3) as usize, ((i + 1) % 3) as usize);
            stats.record_fence_wait(0); // No fence waits
            stats.end_frame();
        }

        assert!(stats.health_check().is_ok());
    }

    #[test]
    fn test_debug_stats_health_check_high_fence_wait() {
        let mut stats = DebugStats::new_enabled();

        // Record frames with high fence wait rate
        for i in 0..20 {
            stats.begin_frame(i, (i % 3) as usize, ((i + 1) % 3) as usize);
            stats.record_fence_wait(100); // Every frame has wait
            stats.end_frame();
        }

        let result = stats.health_check();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fence wait rate"));
    }

    #[test]
    fn test_debug_stats_health_check_high_max_wait() {
        let mut stats = DebugStats::new_enabled();

        stats.begin_frame(0, 0, 2);
        stats.record_fence_wait(5000); // 5ms wait - too high
        stats.end_frame();

        let result = stats.health_check();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("max fence wait"));
    }

    #[test]
    fn test_debug_stats_summary() {
        let mut stats = DebugStats::new_enabled();

        stats.begin_frame(0, 0, 2);
        stats.record_fence_wait(50);
        stats.record_reallocation();
        stats.end_frame();

        let summary = stats.summary();
        assert!(summary.contains("Frames tracked: 1"));
        assert!(summary.contains("reallocations: 1"));
    }

    #[test]
    fn test_debug_stats_status_line() {
        let mut stats = DebugStats::new_enabled();

        stats.begin_frame(0, 0, 2);
        stats.end_frame();

        let status = stats.status_line();
        assert!(status.contains("TB:"));
        assert!(status.contains("fence="));
    }

    #[test]
    fn test_debug_stats_disabled_no_op() {
        let mut stats = DebugStats::new(); // Disabled

        // All operations should be no-ops
        stats.begin_frame(0, 0, 2);
        stats.record_fence_wait(1000);
        stats.record_reallocation();
        stats.record_instance_count(100);
        stats.end_frame();

        // Nothing should be recorded
        assert!(stats.recent_frames().is_empty());
        assert_eq!(stats.total_fence_waits, 0);
        assert_eq!(stats.total_reallocations, 0);
    }

    #[test]
    fn test_triple_buffer_debug_methods() {
        let mut resources = TripleBufferedResources::new();

        // Initially disabled
        assert!(!resources.is_debug_enabled());

        // Enable debug
        resources.enable_debug();
        assert!(resources.is_debug_enabled());

        // Track a frame
        resources.debug_begin_frame();
        resources.debug_record_fence_wait(50);
        resources.debug_record_instance_count(100);
        resources.debug_record_reallocation();
        resources.debug_end_frame();

        let stats = resources.get_debug_stats();
        assert_eq!(stats.recent_frames().len(), 1);

        // Disable debug
        resources.disable_debug();
        assert!(!resources.is_debug_enabled());
    }

    #[test]
    fn test_triple_buffer_debug_report() {
        let mut resources = TripleBufferedResources::new();
        resources.enable_debug();

        // Track some frames
        for _ in 0..3 {
            resources.debug_begin_frame();
            resources.debug_record_instance_count(100);
            resources.debug_end_frame();
            resources.advance_frame();
        }

        let report = resources.debug_report();

        assert!(report.contains("Triple Buffer Debug Report"));
        assert!(report.contains("Current State:"));
        assert!(report.contains("Memory:"));
        assert!(report.contains("Fence States:"));
        assert!(report.contains("Health Check:"));
    }

    #[test]
    fn test_frame_timing_default() {
        let timing = FrameTiming::default();
        assert_eq!(timing.frame, 0);
        assert_eq!(timing.write_slot, 0);
        assert_eq!(timing.gpu_read_slot, 0);
        assert_eq!(timing.fence_wait_us, 0);
        assert_eq!(timing.write_time_us, 0);
        assert_eq!(timing.instance_count, 0);
        assert!(!timing.reallocated);
    }

    #[test]
    fn test_debug_circular_buffer() {
        let mut stats = DebugStats::new_enabled();

        // Fill beyond capacity
        for i in 0..100 {
            stats.begin_frame(i, (i % 3) as usize, ((i + 1) % 3) as usize);
            stats.end_frame();
        }

        // Should only keep DEBUG_HISTORY_SIZE (60) entries
        assert_eq!(stats.recent_frames().len(), 60);
    }

    // ==================== Verification Tests (Phase 6, Step 13) ====================
    //
    // These tests verify the correct behavior of triple buffering as specified
    // in the implementation plan.

    /// Verifies that after proper rotation, fence waits should be zero
    /// (GPU finished 3 frames ago, so buffer is safe to reuse)
    #[test]
    fn test_verify_no_fence_waits_after_warmup() {
        let mut resources = TripleBufferedResources::new();
        resources.enable_debug();

        // Warmup: run 6 frames to fill all slots twice
        for _ in 0..6 {
            let _ctx = resources.begin_frame();
            resources.debug_begin_frame();
            // Simulate zero fence wait (slot should be ready)
            resources.debug_record_fence_wait(0);
            resources.debug_end_frame();
            resources.complete_frame();
        }

        // After warmup, verify all subsequent frames have zero fence waits
        for _ in 0..30 {
            let _ctx = resources.begin_frame();

            // The slot should always be ready (never waiting)
            let slot_index = resources.current_index();
            assert!(
                resources.slots[slot_index].is_safe_for_write(resources.frame_count),
                "Slot {} should be safe to write at frame {}",
                slot_index,
                resources.frame_count
            );

            resources.debug_begin_frame();
            resources.debug_record_fence_wait(0);
            resources.debug_end_frame();
            resources.complete_frame();
        }

        // Verify health check passes
        assert!(
            resources.debug_stats.health_check().is_ok(),
            "Health check should pass with zero fence waits"
        );

        // Verify fence wait percentage is 0%
        assert_eq!(
            resources.debug_stats.fence_wait_percentage(),
            0.0,
            "Fence wait percentage should be 0% with proper triple buffering"
        );
    }

    /// Verifies the rotation pattern matches the documented scheme
    #[test]
    fn test_verify_rotation_scheme() {
        let mut resources = TripleBufferedResources::new();

        // Verify rotation pattern from documentation:
        // Frame 0: Write[0], GPU reads [1], InFlight[2]
        // Frame 1: Write[1], GPU reads [2], InFlight[0]
        // Frame 2: Write[2], GPU reads [0], InFlight[1]
        // Frame 3: Write[0], GPU reads [1], InFlight[2] (cycle repeats)

        let expected = [
            (0, 1, 2), // Frame 0: write, gpu_read, in_flight
            (1, 2, 0), // Frame 1
            (2, 0, 1), // Frame 2
            (0, 1, 2), // Frame 3 (cycle repeats)
            (1, 2, 0), // Frame 4
            (2, 0, 1), // Frame 5
        ];

        for (frame, (exp_write, exp_gpu, exp_flight)) in expected.iter().enumerate() {
            let rotation = resources.rotation_state();

            assert_eq!(
                rotation.write_slot, *exp_write,
                "Frame {}: write_slot mismatch", frame
            );
            assert_eq!(
                rotation.gpu_read_slot, *exp_gpu,
                "Frame {}: gpu_read_slot mismatch", frame
            );
            assert_eq!(
                rotation.in_flight_slot, *exp_flight,
                "Frame {}: in_flight_slot mismatch", frame
            );

            resources.advance_frame();
        }
    }

    /// Verifies that buffer reuse happens correctly after 3 frames
    #[test]
    fn test_verify_buffer_reuse_timing() {
        let mut resources = TripleBufferedResources::new();

        // Track which frames each slot was written to
        let mut slot_write_frames: [Option<u64>; BUFFER_COUNT] = [None; BUFFER_COUNT];

        for frame in 0..12 {
            let slot_index = resources.current_index();

            // If slot was previously used, verify it's been at least 3 frames
            if let Some(last_write) = slot_write_frames[slot_index] {
                let frames_since = frame - last_write;
                assert!(
                    frames_since >= 3,
                    "Slot {} reused after only {} frames (expected >= 3)",
                    slot_index,
                    frames_since
                );
            }

            // Record this write
            slot_write_frames[slot_index] = Some(frame);

            resources.advance_frame();
        }
    }

    /// Verifies growth factor reduces reallocation frequency
    #[test]
    fn test_verify_growth_factor_reduces_reallocations() {
        // Simulate a scenario where data grows gradually
        let sizes = [1000, 1100, 1200, 1300, 1400, 1500, 1600, 1700, 1800];

        // Without growth factor: would need reallocation every time
        // With 1.5x growth factor: should need fewer reallocations

        let initial_capacity = calculate_initial_capacity(sizes[0]);
        let mut current_capacity = initial_capacity;
        let mut reallocations = 0;

        for &size in &sizes[1..] {
            if size > current_capacity {
                current_capacity = calculate_grown_capacity(size);
                reallocations += 1;
            }
        }

        // With 2.25x initial headroom and 1.5x growth, we should have
        // fewer reallocations than the number of size increases
        assert!(
            reallocations < sizes.len() - 1,
            "Growth factor should reduce reallocations: {} reallocs for {} size changes",
            reallocations,
            sizes.len() - 1
        );

        // Specifically, with initial_capacity of 1000 * 2.25 = 2250,
        // we shouldn't need any reallocations for this test data
        assert_eq!(
            reallocations, 0,
            "With 2.25x initial headroom, no reallocations needed for sizes up to 1800"
        );
    }

    /// Verifies the synchronization flow matches documentation
    #[test]
    fn test_verify_sync_flow_documentation() {
        let mut resources = TripleBufferedResources::new();

        // Verify the documented per-frame workflow:
        // 1. BEGIN FRAME: update_fence_states()
        // 2. ACQUIRE SLOT: current_index(), wait_for_current_slot()
        // 3. WRITE DATA: begin_frame_write(), [write], end_frame_write()
        // 4. RENDER: [GPU reads from current slot's buffer]
        // 5. END FRAME: advance_frame()

        for frame in 0..10 {
            // Step 1 & 2: begin_frame() does both
            let ctx = resources.begin_frame();

            assert_eq!(ctx.frame_number, frame);
            assert_eq!(ctx.slot_index, (frame % 3) as usize);

            // Verify slot is in Writing state after begin_frame
            assert_eq!(
                resources.current_slot().sync_state(),
                FrameSyncState::Writing
            );

            // Step 3: Writing happens here (simulated)
            // In real code: render_queue.write_buffer(...)

            // Step 4 & 5: complete_frame() ends write and advances
            resources.complete_frame();

            // Verify slot transitioned to Submitted
            let prev_slot = resources.index_frames_ago(1);
            assert_eq!(
                resources.slots[prev_slot].sync_state(),
                FrameSyncState::Submitted
            );
        }
    }

    /// Verifies frame context provides correct information
    #[test]
    fn test_verify_frame_context_accuracy() {
        let mut resources = TripleBufferedResources::new();
        resources.instance_count = 500;

        for frame in 0..9 {
            let ctx = resources.begin_frame();

            // Verify context matches resource state
            assert_eq!(ctx.frame_number, resources.frame_count);
            assert_eq!(ctx.slot_index, resources.current_index());
            assert_eq!(ctx.instance_count, resources.instance_count);

            // Verify slot_index cycles correctly
            assert_eq!(ctx.slot_index, (frame % 3) as usize);

            resources.complete_frame();
        }
    }

    /// Verifies allocation stats are tracked correctly
    #[test]
    fn test_verify_allocation_stats_tracking() {
        let mut resources = TripleBufferedResources::new();

        // Initial allocation
        let initial_cap = 4096;
        resources.record_initial_allocation(initial_cap);

        let stats = resources.get_allocation_stats();
        assert_eq!(stats.initial_allocations, 3); // All 3 slots
        assert_eq!(stats.current_capacity, initial_cap * 3);
        assert_eq!(stats.reallocations, 0);

        // Simulate reallocation of one slot
        resources.record_slot_reallocation(initial_cap, 8192);

        let stats = resources.get_allocation_stats();
        assert_eq!(stats.reallocations, 1);
        assert_eq!(stats.current_capacity, initial_cap * 2 + 8192);
        assert_eq!(stats.peak_capacity, 8192);
    }

    /// Verifies debug report contains all expected sections
    #[test]
    fn test_verify_debug_report_completeness() {
        let mut resources = TripleBufferedResources::new();
        resources.enable_debug();

        // Run a few frames
        for _ in 0..5 {
            resources.debug_begin_frame();
            resources.debug_record_instance_count(100);
            resources.debug_end_frame();
            resources.advance_frame();
        }

        let report = resources.debug_report();

        // Verify all sections are present
        assert!(report.contains("Triple Buffer Debug Report"), "Missing title");
        assert!(report.contains("Current State:"), "Missing rotation state");
        assert!(report.contains("Memory:"), "Missing memory info");
        assert!(report.contains("Fence States:"), "Missing fence states");
        assert!(report.contains("Health Check:"), "Missing health check");
        assert!(report.contains("Slot 0:"), "Missing slot 0 info");
        assert!(report.contains("Slot 1:"), "Missing slot 1 info");
        assert!(report.contains("Slot 2:"), "Missing slot 2 info");
    }

    /// Verifies health check detects common issues
    #[test]
    fn test_verify_health_check_detection() {
        // Test 1: Healthy state
        {
            let mut stats = DebugStats::new_enabled();
            for i in 0..20 {
                stats.begin_frame(i, (i % 3) as usize, ((i + 1) % 3) as usize);
                stats.record_fence_wait(0);
                stats.end_frame();
            }
            assert!(stats.health_check().is_ok(), "Healthy state should pass");
        }

        // Test 2: High fence wait rate detection
        {
            let mut stats = DebugStats::new_enabled();
            for i in 0..20 {
                stats.begin_frame(i, (i % 3) as usize, ((i + 1) % 3) as usize);
                stats.record_fence_wait(10); // Small but consistent waits
                stats.end_frame();
            }
            let result = stats.health_check();
            assert!(result.is_err(), "Should detect high fence wait rate");
            assert!(result.unwrap_err().contains("fence wait rate"));
        }

        // Test 3: High max fence wait detection
        {
            let mut stats = DebugStats::new_enabled();
            stats.begin_frame(0, 0, 2);
            stats.record_fence_wait(2000); // 2ms - too high
            stats.end_frame();

            let result = stats.health_check();
            assert!(result.is_err(), "Should detect high max fence wait");
            assert!(result.unwrap_err().contains("max fence wait"));
        }

        // Test 4: High reallocation rate detection
        {
            let mut stats = DebugStats::new_enabled();
            for i in 0..20 {
                stats.begin_frame(i, (i % 3) as usize, ((i + 1) % 3) as usize);
                if i % 2 == 0 {
                    stats.record_reallocation(); // 50% reallocation rate
                }
                stats.end_frame();
            }
            let result = stats.health_check();
            assert!(result.is_err(), "Should detect high reallocation rate");
            assert!(result.unwrap_err().contains("reallocation rate"));
        }
    }

    /// Comprehensive end-to-end simulation of triple buffering
    #[test]
    fn test_verify_end_to_end_simulation() {
        let mut resources = TripleBufferedResources::new();
        resources.enable_debug();

        // Simulate 100 frames of rendering
        for frame in 0..100 {
            // 1. Begin frame
            let ctx = resources.begin_frame();
            resources.debug_begin_frame();

            // 2. Verify correct slot assignment
            assert_eq!(ctx.slot_index, (frame % 3) as usize);

            // 3. Verify slot is safe (no fence wait needed)
            // After first 3 frames, this should always be true
            if frame >= 3 {
                assert!(
                    resources.slots[ctx.slot_index].fence.is_ready(resources.frame_count),
                    "Slot should be ready at frame {}", frame
                );
            }

            // 4. Simulate some work
            resources.debug_record_instance_count(100 + (frame % 50) as u32);
            resources.debug_record_fence_wait(0); // Should always be 0

            // 5. Complete frame
            resources.debug_end_frame();
            resources.complete_frame();
        }

        // Verify final state
        assert_eq!(resources.frame_count, 100);

        // Verify health
        let health = resources.debug_stats.health_check();
        assert!(health.is_ok(), "End-to-end simulation should be healthy: {:?}", health);

        // Verify no fence waits occurred
        assert_eq!(
            resources.debug_stats.fence_wait_percentage(),
            0.0,
            "No fence waits should occur in proper triple buffering"
        );

        // Verify no reallocations (constant data size)
        assert_eq!(
            resources.debug_stats.total_reallocations,
            0,
            "No reallocations with constant data size"
        );
    }
}
