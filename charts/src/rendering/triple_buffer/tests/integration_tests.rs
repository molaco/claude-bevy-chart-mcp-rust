//! Integration and end-to-end tests for triple buffering.

use crate::rendering::triple_buffer::{FrameSyncState, TripleBufferedResources, BUFFER_COUNT};

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
                resources.slots[ctx.slot_index]
                    .fence
                    .is_ready(resources.frame_count),
                "Slot should be ready at frame {}",
                frame
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
    assert!(
        health.is_ok(),
        "End-to-end simulation should be healthy: {:?}",
        health
    );

    // Verify no fence waits occurred
    assert_eq!(
        resources.debug_stats.fence_wait_percentage(),
        0.0,
        "No fence waits should occur in proper triple buffering"
    );

    // Verify no reallocations (constant data size)
    assert_eq!(
        resources.debug_stats.total_reallocations, 0,
        "No reallocations with constant data size"
    );
}

/// Test concurrent operations across multiple frame cycles
#[test]
fn test_multi_cycle_operations() {
    let mut resources = TripleBufferedResources::new();

    // Run 30 complete cycles (90 frames)
    for cycle in 0..30 {
        for slot in 0..BUFFER_COUNT {
            let frame = cycle * BUFFER_COUNT as u64 + slot as u64;

            let ctx = resources.begin_frame();

            // Verify we're on the expected frame and slot
            assert_eq!(ctx.frame_number, frame);
            assert_eq!(ctx.slot_index, slot);

            // Verify current slot is being written
            assert_eq!(
                resources.current_slot().sync_state(),
                FrameSyncState::Writing
            );

            resources.complete_frame();

            // Verify the slot we just wrote is now submitted
            assert_eq!(resources.slots[slot].sync_state(), FrameSyncState::Submitted);
        }
    }

    assert_eq!(resources.frame_count, 90);
}

/// Test that fence states update correctly after multiple frames
#[test]
fn test_fence_state_progression() {
    let mut resources = TripleBufferedResources::new();

    // Write to all 3 slots without calling begin_frame (which updates fences)
    // to keep precise control over fence states
    for _ in 0..3 {
        resources.begin_frame_write();
        resources.end_frame_write();
        resources.advance_frame();
    }

    // At frame 3: slot 0 submitted at frame 0 (3-0=3 >= 2, so Complete)
    //             slot 1 submitted at frame 1 (3-1=2 >= 2, so Complete)
    //             slot 2 submitted at frame 2 (3-2=1 < 2, so still Submitted)
    resources.update_fence_states();

    // Only slot 2 should still be Submitted (it was just submitted 1 frame ago)
    assert_eq!(
        resources.slots[2].sync_state(),
        FrameSyncState::Submitted,
        "Slot 2 should still be Submitted (only 1 frame ago)"
    );

    // Slots 0 and 1 should be Complete
    assert_eq!(
        resources.slots[0].sync_state(),
        FrameSyncState::Complete,
        "Slot 0 should be Complete (3 frames ago)"
    );
    assert_eq!(
        resources.slots[1].sync_state(),
        FrameSyncState::Complete,
        "Slot 1 should be Complete (2 frames ago)"
    );

    // Advance one more frame
    resources.advance_frame();
    resources.update_fence_states();

    // Now all slots should be Complete (slot 2 was submitted at frame 2, now frame 4, 4-2=2 >= 2)
    for (i, slot) in resources.slots.iter().enumerate() {
        assert_eq!(
            slot.sync_state(),
            FrameSyncState::Complete,
            "Slot {} should be Complete after all fences updated",
            i
        );
    }
}

/// Test instance count propagation through frames
#[test]
fn test_instance_count_propagation() {
    let mut resources = TripleBufferedResources::new();

    let instance_counts = [100, 250, 500, 1000, 750, 300];

    for &count in &instance_counts {
        resources.instance_count = count;
        let ctx = resources.begin_frame();

        assert_eq!(
            ctx.instance_count, count,
            "Instance count should be {} in frame context",
            count
        );

        resources.complete_frame();
    }
}

/// Test rapid frame cycling doesn't cause issues
#[test]
fn test_rapid_frame_cycling() {
    let mut resources = TripleBufferedResources::new();

    // Rapid cycling through 1000 frames
    for _ in 0..1000 {
        let ctx = resources.begin_frame();
        assert!(ctx.slot_index < BUFFER_COUNT);
        resources.complete_frame();
    }

    assert_eq!(resources.frame_count, 1000);
}

/// Test that slot reuse timing is correct even with frame skips
#[test]
fn test_slot_reuse_with_skipped_frames() {
    let mut resources = TripleBufferedResources::new();

    // Write to slot 0
    let _ctx = resources.begin_frame();
    resources.complete_frame();

    // Skip several frames
    for _ in 0..5 {
        resources.skip_frame();
    }

    // At frame 6, slot 0 should be safe
    assert_eq!(resources.current_index(), 0);
    assert!(
        resources.current_slot_is_safe(),
        "Slot 0 should be safe after 5 skipped frames"
    );
}

/// Test memory tracking consistency
#[test]
fn test_memory_tracking_consistency() {
    let mut resources = TripleBufferedResources::new();

    // Set consistent capacities
    for slot in &mut resources.slots {
        slot.buffer_capacity = 4096;
        slot.uniform_buffer_capacity = 256;
    }

    // Record initial allocation
    resources.record_initial_allocation(4096);

    // Verify memory tracking
    assert_eq!(resources.total_instance_buffer_memory(), 3 * 4096);
    assert_eq!(resources.total_uniform_buffer_memory(), 3 * 256);

    let stats = resources.get_allocation_stats();
    assert_eq!(stats.initial_allocations, 3);
    assert_eq!(stats.current_capacity, 3 * 4096);

    // Simulate a reallocation
    resources.slots[0].buffer_capacity = 8192;
    resources.record_slot_reallocation(4096, 8192);

    // Verify updated tracking
    assert_eq!(
        resources.total_instance_buffer_memory(),
        8192 + 4096 + 4096
    );
    let stats = resources.get_allocation_stats();
    assert_eq!(stats.reallocations, 1);
}

/// Test debug stats integration with frame workflow
#[test]
fn test_debug_stats_with_workflow() {
    let mut resources = TripleBufferedResources::new();
    resources.enable_debug();

    for frame in 0..20 {
        let _ctx = resources.begin_frame();
        resources.debug_begin_frame();

        resources.debug_record_instance_count((frame * 10 + 100) as u32);

        // Simulate occasional reallocation
        if frame == 5 {
            resources.debug_record_reallocation();
        }

        resources.debug_end_frame();
        resources.complete_frame();
    }

    let stats = resources.get_debug_stats();

    assert_eq!(stats.recent_frames().len(), 20);
    assert_eq!(stats.total_reallocations, 1);
    assert_eq!(stats.total_fence_waits, 0);
}

/// Test that preallocated check works correctly
#[test]
fn test_preallocated_check() {
    let resources = TripleBufferedResources::new();

    // Initially not preallocated
    assert!(!resources.is_preallocated());

    // Note: We can't fully test preallocated with actual buffers here
    // since we don't have access to a GPU device in unit tests.
    // The logic is tested via the needs_reallocation methods.
}

/// Test any_slot_needs_reallocation
#[test]
fn test_any_slot_needs_reallocation() {
    let mut resources = TripleBufferedResources::new();

    // All slots start empty, so any size needs reallocation
    assert!(resources.any_slot_needs_reallocation(100));

    // Set one slot's capacity
    resources.slots[0].buffer_capacity = 1000;
    // Still needs reallocation because buffer is None
    assert!(resources.any_slot_needs_reallocation(100));
}

/// Test invalidate_all_bind_groups
#[test]
fn test_invalidate_all_bind_groups() {
    let mut resources = TripleBufferedResources::new();

    // Set all bind groups as valid
    for slot in &mut resources.slots {
        slot.bind_group_valid = true;
    }

    // Verify they're valid
    assert!(resources.slots.iter().all(|s| s.bind_group_valid));

    // Invalidate all
    resources.invalidate_all_bind_groups();

    // Verify they're all invalid
    assert!(resources.slots.iter().all(|s| !s.bind_group_valid));
}

/// Test track_instance_count updates allocation stats
#[test]
fn test_track_instance_count() {
    let mut resources = TripleBufferedResources::new();

    resources.track_instance_count(100);
    assert_eq!(
        resources.get_allocation_stats().peak_instance_count,
        100
    );

    resources.track_instance_count(50);
    assert_eq!(
        resources.get_allocation_stats().peak_instance_count,
        100
    ); // Peak unchanged

    resources.track_instance_count(200);
    assert_eq!(
        resources.get_allocation_stats().peak_instance_count,
        200
    ); // New peak
}
