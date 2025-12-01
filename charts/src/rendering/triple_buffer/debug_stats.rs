//! Debug statistics for triple buffer performance monitoring.
//!
//! This module provides types for tracking timing information and
//! identifying synchronization issues.

use std::time::Instant;

use super::constants::DEBUG_HISTORY_SIZE;

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
