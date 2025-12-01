//! Tests for core rotation and index calculation.

use crate::rendering::triple_buffer::{
    FrameSlot, RotationState, TripleBufferedResources, BUFFER_COUNT,
};

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
    assert_eq!(
        initial_state.in_flight_slot,
        after_cycle_state.in_flight_slot
    );
    assert_eq!(after_cycle_state.frame, 3);
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
            "Frame {}: write_slot mismatch",
            frame
        );
        assert_eq!(
            rotation.gpu_read_slot, *exp_gpu,
            "Frame {}: gpu_read_slot mismatch",
            frame
        );
        assert_eq!(
            rotation.in_flight_slot, *exp_flight,
            "Frame {}: in_flight_slot mismatch",
            frame
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
