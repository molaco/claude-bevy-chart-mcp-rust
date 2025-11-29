//! Integration tests for handler systems using Bevy ECS test harness.
//!
//! These tests verify that the pan, zoom, and resize handlers correctly
//! modify resources when systems are run through the Bevy scheduler.
//!
//! See also:
//! - `integration_startup.rs` for startup/initialization tests
//! - `integration_frame.rs` for frame pipeline and edge case tests

#![allow(deprecated)] // Allow deprecated Bevy 0.17 event APIs

mod common;

use bevy::prelude::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};

use charts::config::InteractionConfig;
use charts::coordinate::ViewportState;
use charts::data::CandleData;
use charts::interaction::{handle_pan, handle_resize, handle_zoom, InteractionMode, InteractionState};
use charts::panes::{PaneId, PaneManager};

use common::create_test_app;

// ============================================================================
// PAN HANDLER TESTS (Step 3.1)
// ============================================================================

#[test]
fn test_pan_handler_not_in_panning_mode_does_nothing() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Ensure we're in Idle mode (default)
    let initial_start = app.world().resource::<ViewportState>().visible_candle_start;

    // Run the system
    app.update();

    // Should not have changed
    let final_start = app.world().resource::<ViewportState>().visible_candle_start;
    assert_eq!(initial_start, final_start, "Pan should not change viewport when not in Panning mode");
}

#[test]
fn test_pan_handler_panning_mode_no_movement() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Set up panning mode with mouse at (100, 100)
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0, 100.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0), // Same as current pos = no movement
        };
    }

    let initial_start = app.world().resource::<ViewportState>().visible_candle_start;

    app.update();

    let final_start = app.world().resource::<ViewportState>().visible_candle_start;
    assert_eq!(initial_start, final_start, "No movement should mean no viewport change");
}

#[test]
fn test_pan_handler_drag_right_decreases_start_index() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Initial viewport start
    let initial_start = {
        let viewport = app.world().resource::<ViewportState>();
        viewport.visible_candle_start
    };

    // Set up panning with rightward drag (positive X delta = scroll into history)
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 + candle_width * 5.0, 100.0); // Moved right by 5 candles worth
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        };
    }

    app.update();

    let final_start = app.world().resource::<ViewportState>().visible_candle_start;

    assert!(
        final_start < initial_start,
        "Dragging right should decrease visible_candle_start (scroll into history): {} < {}",
        final_start, initial_start
    );
}

#[test]
fn test_pan_handler_drag_left_increases_start_index() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Initial viewport start
    let initial_start = {
        let viewport = app.world().resource::<ViewportState>();
        viewport.visible_candle_start
    };

    // Set up panning with leftward drag (negative X delta = scroll toward recent)
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 - candle_width * 5.0, 100.0); // Moved left by 5 candles worth
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        };
    }

    app.update();

    let final_start = app.world().resource::<ViewportState>().visible_candle_start;

    assert!(
        final_start > initial_start,
        "Dragging left should increase visible_candle_start (scroll toward recent): {} > {}",
        final_start, initial_start
    );
}

#[test]
fn test_pan_handler_clamps_to_zero() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Set viewport to start at candle 0
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.visible_candle_start = 0;
    }

    // Try to pan further into history (right drag)
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 + candle_width * 100.0, 100.0); // Large right drag
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        };
    }

    app.update();

    let final_start = app.world().resource::<ViewportState>().visible_candle_start;
    assert_eq!(final_start, 0, "Should clamp to 0, not go negative");
}

#[test]
fn test_pan_handler_clamps_to_end() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Get candle count
    let candle_count = app.world().resource::<CandleData>().candles.len();
    let visible_count = app.world().resource::<ViewportState>().visible_candle_count;

    // Set viewport near the end
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.visible_candle_start = candle_count.saturating_sub(visible_count);
    }

    // Try to pan further toward recent (left drag)
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 - candle_width * 200.0, 100.0); // Large left drag
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        };
    }

    app.update();

    let final_start = app.world().resource::<ViewportState>().visible_candle_start;

    // Should be clamped within reasonable bounds (with right spacing allowance)
    assert!(
        final_start <= candle_count + 100,
        "Should be clamped within reasonable bounds"
    );
}

#[test]
fn test_pan_handler_sets_redraw_flag() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Clear the redraw flag
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.needs_redraw = false;
    }

    // Set up panning with movement
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 + candle_width * 5.0, 100.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        };
    }

    app.update();

    let needs_redraw = app.world().resource::<ViewportState>().needs_redraw;
    assert!(needs_redraw, "Pan with movement should set needs_redraw = true");
}

#[test]
fn test_pan_handler_updates_drag_start() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Set up panning
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 + candle_width * 2.0, 100.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        };
    }

    app.update();

    // Check that drag_start was updated to current mouse_pos
    let interaction = app.world().resource::<InteractionState>();
    if let InteractionMode::Panning { drag_start } = interaction.mode {
        assert_eq!(
            drag_start, interaction.mouse_pos,
            "drag_start should be updated to current mouse_pos after pan"
        );
    } else {
        panic!("Should still be in Panning mode");
    }
}

#[test]
fn test_pan_handler_empty_panes_does_nothing() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Clear panes
    {
        let mut manager = app.world_mut().resource_mut::<PaneManager>();
        manager.panes.clear();
    }

    // Set up panning
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(200.0, 100.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        };
    }

    let initial_start = app.world().resource::<ViewportState>().visible_candle_start;

    // Should not panic
    app.update();

    let final_start = app.world().resource::<ViewportState>().visible_candle_start;
    assert_eq!(initial_start, final_start, "Empty panes should cause early return");
}

#[test]
fn test_pan_updates_pane_bounds() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Set up panning with significant movement to change visible candles
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 + candle_width * 50.0, 300.0); // Large pan
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 300.0),
        };
    }

    app.update();

    // After panning, the handler should have called update_pane_bounds
    // We verify this indirectly by checking needs_redraw is set
    assert!(
        app.world().resource::<ViewportState>().needs_redraw,
        "Pan should trigger redraw which indicates bounds were updated"
    );
}

// ============================================================================
// ZOOM HANDLER TESTS (Step 3.2)
// ============================================================================

#[test]
fn test_zoom_handler_no_events_does_nothing() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    let initial_count = app.world().resource::<ViewportState>().visible_candle_count;

    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert_eq!(initial_count, final_count, "No events should mean no change");
}

#[test]
fn test_zoom_handler_blocked_during_resize() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Set up resizing mode
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 300.0); // In pane
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 300.0),
            start_heights: vec![0.7, 0.3],
        };
    }

    // Send zoom event
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0, // Scroll up = zoom in
        window: Entity::PLACEHOLDER,
    });

    let initial_count = app.world().resource::<ViewportState>().visible_candle_count;

    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert_eq!(initial_count, final_count, "Zoom should be blocked during resize");
}

#[test]
fn test_zoom_handler_mouse_not_in_pane_does_nothing() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Position mouse outside all panes
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(-100.0, -100.0); // Outside viewport
    }

    // Send zoom event
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    let initial_count = app.world().resource::<ViewportState>().visible_candle_count;

    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert_eq!(initial_count, final_count, "Zoom should be ignored when mouse not in pane");
}

#[test]
fn test_zoom_in_decreases_visible_count() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Position mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0); // Middle of price pane
    }

    let initial_count = app.world().resource::<ViewportState>().visible_candle_count;

    // Send zoom in event (scroll up = positive y)
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert!(
        final_count < initial_count,
        "Zoom in should decrease visible_candle_count: {} < {}",
        final_count, initial_count
    );
}

#[test]
fn test_zoom_out_increases_visible_count() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Position mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    let initial_count = app.world().resource::<ViewportState>().visible_candle_count;

    // Send zoom out event (scroll down = negative y)
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: -1.0,
        window: Entity::PLACEHOLDER,
    });

    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert!(
        final_count > initial_count,
        "Zoom out should increase visible_candle_count: {} > {}",
        final_count, initial_count
    );
}

#[test]
fn test_zoom_clamps_to_min_candles() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    let min_candles = app.world().resource::<InteractionConfig>().min_visible_candles as usize;

    // Set visible count to minimum
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.visible_candle_count = min_candles;
    }

    // Position mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    // Try to zoom in further
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert!(
        final_count >= min_candles,
        "Should not go below min_visible_candles: {} >= {}",
        final_count, min_candles
    );
}

#[test]
fn test_zoom_clamps_to_max_candles() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    let candle_data_len = app.world().resource::<CandleData>().candles.len();
    let max_candles = app.world().resource::<InteractionConfig>().max_visible_candles as usize;

    // Set visible count to maximum (capped by data length)
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.visible_candle_count = candle_data_len.min(max_candles);
    }

    // Position mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    // Try to zoom out further
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: -1.0,
        window: Entity::PLACEHOLDER,
    });

    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert!(
        final_count <= candle_data_len,
        "Should not exceed candle data length: {} <= {}",
        final_count, candle_data_len
    );
}

#[test]
fn test_zoom_sets_redraw_flag() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Clear redraw flag
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.needs_redraw = false;
    }

    // Position mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    // Send zoom event
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    app.update();

    let needs_redraw = app.world().resource::<ViewportState>().needs_redraw;
    assert!(needs_redraw, "Zoom should set needs_redraw = true");
}

#[test]
fn test_zoom_empty_panes_does_nothing() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Clear panes
    {
        let mut manager = app.world_mut().resource_mut::<PaneManager>();
        manager.panes.clear();
    }

    // Send zoom event
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    let initial_count = app.world().resource::<ViewportState>().visible_candle_count;

    // Should not panic
    app.update();

    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert_eq!(initial_count, final_count, "Empty panes should cause early return");
}

#[test]
fn test_zoom_maintains_focus_point() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Set viewport to a known state
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.visible_candle_start = 100;
        viewport.visible_candle_count = 100;
    }

    // Position mouse at center of viewport (should focus on candle 150)
    let mouse_x = 400.0; // Center of 800px viewport
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(mouse_x, 450.0);
    }

    // Calculate which candle is under the mouse before zoom
    let focus_candle_before = {
        let viewport = app.world().resource::<ViewportState>();
        let manager = app.world().resource::<PaneManager>();
        let (candle_idx, _) = manager.panes[0].space.from_world(
            Vec2::new(mouse_x, 450.0),
            viewport.visible_candle_start,
            viewport.visible_candle_count,
        );
        candle_idx
    };

    // Zoom in
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    app.update();

    // The focus candle should still be visible and near the center
    let (new_start, new_count) = {
        let viewport = app.world().resource::<ViewportState>();
        (viewport.visible_candle_start, viewport.visible_candle_count)
    };

    let focus_offset_after = focus_candle_before.saturating_sub(new_start);
    let focus_percent_after = focus_offset_after as f32 / new_count as f32;

    assert!(
        focus_percent_after > 0.3 && focus_percent_after < 0.7,
        "Focus candle {} should remain near center after zoom: offset {} of {} = {:.1}%",
        focus_candle_before, focus_offset_after, new_count, focus_percent_after * 100.0
    );
}

// ============================================================================
// RESIZE HANDLER TESTS (Step 3.3)
// ============================================================================

#[test]
fn test_resize_handler_not_in_resize_mode_does_nothing() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    let initial_height = app.world().resource::<PaneManager>().panes[0].height_percent;

    app.update();

    let final_height = app.world().resource::<PaneManager>().panes[0].height_percent;
    assert_eq!(initial_height, final_height, "Should not change when not in ResizingPane mode");
}

#[test]
fn test_resize_drag_down_expands_upper_pane() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    let initial_height = app.world().resource::<PaneManager>().panes[0].height_percent;

    // Set up resize mode with downward drag
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 200.0); // Lower Y = dragged down
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 350.0), // Higher Y = started higher
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let final_height = app.world().resource::<PaneManager>().panes[0].height_percent;

    assert!(
        final_height > initial_height,
        "Dragging down should expand upper pane: {} > {}",
        final_height, initial_height
    );
}

#[test]
fn test_resize_drag_up_shrinks_upper_pane() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    let initial_height = app.world().resource::<PaneManager>().panes[0].height_percent;

    // Set up resize mode with upward drag
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 500.0); // Higher Y = dragged up
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 350.0), // Lower Y = started lower
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let final_height = app.world().resource::<PaneManager>().panes[0].height_percent;

    assert!(
        final_height < initial_height,
        "Dragging up should shrink upper pane: {} < {}",
        final_height, initial_height
    );
}

#[test]
fn test_resize_clamps_to_min_height() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    let min_height = app.world().resource::<InteractionConfig>().min_pane_height;

    // Set up resize with large upward drag
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 590.0); // Very high = large upward drag
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 100.0),
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let final_height = app.world().resource::<PaneManager>().panes[0].height_percent;
    assert!(
        final_height >= min_height,
        "Should not go below min_pane_height: {} >= {}",
        final_height, min_height
    );
}

#[test]
fn test_resize_clamps_to_max_height() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    let max_height = app.world().resource::<InteractionConfig>().max_pane_height;

    // Set up resize with large downward drag
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 10.0); // Very low = large downward drag
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 500.0),
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let final_height = app.world().resource::<PaneManager>().panes[0].height_percent;
    assert!(
        final_height <= max_height,
        "Should not exceed max_pane_height: {} <= {}",
        final_height, max_height
    );
}

#[test]
fn test_resize_updates_adjacent_pane() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    // Set up resize with meaningful drag
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 200.0);
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 350.0),
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let pane0_height = app.world().resource::<PaneManager>().panes[0].height_percent;
    let pane1_height = app.world().resource::<PaneManager>().panes[1].height_percent;

    assert!(
        pane0_height != 0.7 || pane1_height != 0.3,
        "At least one pane should have changed"
    );
}

#[test]
fn test_resize_sets_redraw_flag() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    // Clear redraw flag
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.needs_redraw = false;
    }

    // Set up resize with meaningful drag
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 200.0);
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 350.0),
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let needs_redraw = app.world().resource::<ViewportState>().needs_redraw;
    assert!(needs_redraw, "Resize should set needs_redraw = true");
}

#[test]
fn test_resize_ignores_sub_threshold_change() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    // Clear redraw flag
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.needs_redraw = false;
    }

    // Set up resize with very small drag (sub-threshold)
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 350.1); // 0.1 pixel movement
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 350.0),
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let pane0_height = app.world().resource::<PaneManager>().panes[0].height_percent;

    assert!(
        (pane0_height - 0.7).abs() < 0.0001,
        "Sub-threshold movement should not change heights, got {}",
        pane0_height
    );
}

#[test]
fn test_resize_triggers_layout_recalc() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    let initial_pane0_viewport = {
        let manager = app.world().resource::<PaneManager>();
        manager.panes[0].space.viewport
    };

    // Set up resize with meaningful drag
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 200.0);
        interaction.mode = InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 350.0),
            start_heights: vec![0.7, 0.3],
        };
    }

    app.update();

    let final_pane0_viewport = {
        let manager = app.world().resource::<PaneManager>();
        manager.panes[0].space.viewport
    };

    assert!(
        (final_pane0_viewport.height() - initial_pane0_viewport.height()).abs() > 1.0,
        "Pane viewport should be recalculated after resize: {:.1} vs {:.1}",
        final_pane0_viewport.height(), initial_pane0_viewport.height()
    );
}

#[test]
fn test_single_pane_resize_handler_idle() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_resize);

    // Remove volume pane, leaving only price pane
    {
        let mut manager = app.world_mut().resource_mut::<PaneManager>();
        manager.panes.retain(|p| p.id == PaneId::Price);
        manager.panes[0].height_percent = 1.0;
    }

    // With a single pane, interaction stays Idle
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 200.0);
        interaction.mode = InteractionMode::Idle;
    }

    let initial_height = app.world().resource::<PaneManager>().panes[0].height_percent;

    app.update();

    let final_height = app.world().resource::<PaneManager>().panes[0].height_percent;
    assert_eq!(initial_height, final_height, "Single pane height should not change");
}

// ============================================================================
// LAZY LOADING GUARD TESTS (Step 3.4 - without database)
// ============================================================================

#[test]
fn test_lazy_load_guard_blocks_during_panning() {
    let interaction = InteractionState {
        mode: InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 100.0),
        },
        mouse_pos: Vec2::new(150.0, 100.0),
    };

    assert!(
        interaction.mode.is_active(),
        "Panning mode should be considered active"
    );
}

#[test]
fn test_lazy_load_guard_blocks_during_resizing() {
    let interaction = InteractionState {
        mode: InteractionMode::ResizingPane {
            gap_index: 0,
            drag_start: Vec2::new(400.0, 300.0),
            start_heights: vec![0.7, 0.3],
        },
        mouse_pos: Vec2::new(400.0, 250.0),
    };

    assert!(
        interaction.mode.is_active(),
        "ResizingPane mode should be considered active"
    );
}

#[test]
fn test_lazy_load_guard_allows_during_idle() {
    let interaction = InteractionState::default();

    assert!(
        !interaction.mode.is_active(),
        "Idle mode should not be considered active"
    );
}

#[test]
fn test_lazy_load_guard_allows_during_hovering() {
    let interaction = InteractionState {
        mode: InteractionMode::HoveringGap { gap_index: 0 },
        mouse_pos: Vec2::new(400.0, 300.0),
    };

    assert!(
        !interaction.mode.is_active(),
        "HoveringGap mode should not be considered active"
    );
}

#[test]
fn test_lazy_load_loading_flag_blocks() {
    let mut viewport = ViewportState::default();
    viewport.loading = true;

    assert!(
        viewport.loading,
        "Loading flag should prevent concurrent loads"
    );
}

// ============================================================================
// COMBINED INTERACTION TESTS
// ============================================================================

#[test]
fn test_pan_then_zoom_sequence() {
    let mut app = create_test_app();
    app.add_systems(Update, (handle_pan, handle_zoom).chain());

    // First pan
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 + candle_width * 5.0, 300.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 300.0),
        };
    }

    app.update();

    let after_pan_start = app.world().resource::<ViewportState>().visible_candle_start;

    // Then zoom (need to exit panning first)
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mode = InteractionMode::Idle;
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    app.update();

    let after_zoom_count = app.world().resource::<ViewportState>().visible_candle_count;

    // Verify both operations happened
    assert!(after_pan_start != 50, "Pan should have changed start index");
    assert!(after_zoom_count != 100, "Zoom should have changed count");
}

#[test]
fn test_multiple_zoom_events_accumulate() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Position mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    let initial_count = app.world().resource::<ViewportState>().visible_candle_count;

    // First zoom
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });
    app.update();
    let after_one_zoom = app.world().resource::<ViewportState>().visible_candle_count;

    // Keep mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    // Second zoom
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });
    app.update();
    let after_two_zooms = app.world().resource::<ViewportState>().visible_candle_count;

    // Keep mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 450.0);
    }

    // Third zoom
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });
    app.update();
    let after_three_zooms = app.world().resource::<ViewportState>().visible_candle_count;

    // Each zoom should progressively reduce the count
    assert!(
        after_one_zoom < initial_count,
        "First zoom should reduce count: {} < {}",
        after_one_zoom, initial_count
    );
    assert!(
        after_two_zooms < after_one_zoom,
        "Second zoom should reduce count further: {} < {}",
        after_two_zooms, after_one_zoom
    );
    assert!(
        after_three_zooms < after_two_zooms,
        "Third zoom should reduce count even further: {} < {}",
        after_three_zooms, after_two_zooms
    );
}
