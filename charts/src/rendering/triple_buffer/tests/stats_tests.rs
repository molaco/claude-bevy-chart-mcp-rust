//! Tests for AllocationStats, DebugStats, and health checks.

use crate::rendering::triple_buffer::{
    calculate_grown_capacity, calculate_initial_capacity, format_bytes, AllocationStats,
    DebugStats, FrameTiming, TripleBufferedResources, BUFFER_GROWTH_FACTOR, MIN_BUFFER_CAPACITY,
};

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
    let expected =
        (required as f32 * BUFFER_GROWTH_FACTOR * BUFFER_GROWTH_FACTOR).ceil() as usize;
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
    // start_time is private, but we verify it's set by checking that
    // the constructor worked (enabled is true implies start_time is set)
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
