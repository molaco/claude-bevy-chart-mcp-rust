//! Allocation statistics for buffer memory tracking.
//!
//! This module provides types for tracking buffer allocation patterns,
//! useful for debugging and performance optimization.

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
        self.current_capacity += capacity; // Accumulate total capacity
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
