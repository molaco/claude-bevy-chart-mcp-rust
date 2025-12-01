//! Tests for synchronization primitives (FrameSyncState, FrameFence, slot acquisition).

use crate::rendering::triple_buffer::{
    FrameContext, FrameFence, FrameSlot, FrameSyncState, SlotAcquisition, TripleBufferedResources,
};

// ==================== FrameSyncState Tests ====================

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

// ==================== FrameFence Tests ====================

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
    assert_eq!(
        resources.current_slot().sync_state(),
        FrameSyncState::Writing
    );
    resources.end_frame_write();
    assert_eq!(
        resources.current_slot().sync_state(),
        FrameSyncState::Submitted
    );
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
    assert!(resources
        .slots
        .iter()
        .all(|s| s.fence.submission_frame.is_some()));

    // Reset all
    resources.reset_all_fences();

    // All should be unused now
    assert!(resources
        .slots
        .iter()
        .all(|s| s.fence.state == FrameSyncState::Unused));
    assert!(resources
        .slots
        .iter()
        .all(|s| s.fence.submission_frame.is_none()));
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
        SlotAcquisition::Busy {
            slot_index,
            frames_remaining,
        } => {
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
    assert_eq!(
        resources.current_slot().sync_state(),
        FrameSyncState::Writing
    );
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

    let busy = SlotAcquisition::Busy {
        slot_index: 2,
        frames_remaining: 1,
    };
    assert_eq!(busy.slot_index(), 2);
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
