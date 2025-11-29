//! Integration tests for frame pipeline and edge cases.
//!
//! These tests verify the frame-by-frame execution of systems,
//! cleanup behavior, and edge case handling.

#![allow(deprecated)] // Allow deprecated Bevy 0.17 event APIs

mod common;

use bevy::prelude::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};

use charts::coordinate::ViewportState;
use charts::data::CandleData;
use charts::interaction::{handle_pan, handle_zoom, InteractionMode, InteractionState};
use charts::panes::{PaneId, PaneManager};

use common::create_test_app;

// ============================================================================
// FRAME PIPELINE TESTS
// ============================================================================

#[test]
fn test_frame_cleanup_resets_flags() {
    let mut app = create_test_app();

    // Define a cleanup system similar to the real one
    fn reset_redraw_flag(mut viewport: ResMut<ViewportState>) {
        viewport.needs_redraw = false;
    }

    app.add_systems(Update, reset_redraw_flag);

    // Set needs_redraw to true
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.needs_redraw = true;
    }

    app.update();

    // After cleanup, needs_redraw should be false
    let needs_redraw = app.world().resource::<ViewportState>().needs_redraw;
    assert!(
        !needs_redraw,
        "Cleanup system should reset needs_redraw to false"
    );
}

#[test]
fn test_frame_pan_updates_viewport() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    let initial_start = app.world().resource::<ViewportState>().visible_candle_start;

    // Simulate pan input
    {
        let pane_manager = app.world().resource::<PaneManager>();
        let candle_width = pane_manager.panes[0].space.candle_width_px;

        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(100.0 - candle_width * 10.0, 300.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 300.0),
        };
    }

    app.update();

    let final_start = app.world().resource::<ViewportState>().visible_candle_start;

    assert_ne!(
        initial_start, final_start,
        "Pan input should update viewport state"
    );
}

#[test]
fn test_frame_multiple_updates_stable() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Run multiple frames without input changes
    for _ in 0..10 {
        app.update();
    }

    // State should remain stable
    let viewport = app.world().resource::<ViewportState>();
    assert_eq!(
        viewport.visible_candle_start, 50,
        "Viewport should remain stable without input"
    );
}

// ============================================================================
// EDGE CASE TESTS - Empty Data
// ============================================================================

#[test]
fn test_empty_candle_data_pan_no_crash() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Clear all candles
    {
        let mut candle_data = app.world_mut().resource_mut::<CandleData>();
        candle_data.candles.clear();
    }

    // Set up panning
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(200.0, 300.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 300.0),
        };
    }

    // Should not panic
    app.update();
}

#[test]
fn test_empty_candle_data_zoom_no_crash() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Clear all candles
    {
        let mut candle_data = app.world_mut().resource_mut::<CandleData>();
        candle_data.candles.clear();
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

    // Should not panic
    app.update();
}

// ============================================================================
// EDGE CASE TESTS - Single Candle
// ============================================================================

#[test]
fn test_single_candle_pan_no_crash() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Reduce to single candle
    {
        let mut candle_data = app.world_mut().resource_mut::<CandleData>();
        candle_data.candles.truncate(1);
    }

    // Set up panning
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(200.0, 300.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 300.0),
        };
    }

    // Should not panic
    app.update();
}

#[test]
fn test_single_candle_zoom_no_crash() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Reduce to single candle
    {
        let mut candle_data = app.world_mut().resource_mut::<CandleData>();
        candle_data.candles.truncate(1);
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

    // Should not panic
    app.update();
}

// ============================================================================
// EDGE CASE TESTS - Single Pane
// ============================================================================

#[test]
fn test_single_pane_pan_no_crash() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Remove volume pane, leaving only price pane
    {
        let mut manager = app.world_mut().resource_mut::<PaneManager>();
        manager.panes.retain(|p| p.id == PaneId::Price);
        manager.panes[0].height_percent = 1.0;
    }

    // Set up panning
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(200.0, 300.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 300.0),
        };
    }

    // Should not panic
    app.update();
}

#[test]
fn test_single_pane_zoom_no_crash() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    // Remove volume pane
    {
        let mut manager = app.world_mut().resource_mut::<PaneManager>();
        manager.panes.retain(|p| p.id == PaneId::Price);
    }

    // Position mouse in pane
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(400.0, 300.0);
    }

    // Send zoom event
    app.world_mut().send_event(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
    });

    // Should not panic
    app.update();
}

// ============================================================================
// EDGE CASE TESTS - Extreme Viewport Values
// ============================================================================

#[test]
fn test_zero_visible_candles_no_crash() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_pan);

    // Set visible count to 0 (edge case)
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.visible_candle_count = 0;
    }

    // Set up panning
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mouse_pos = Vec2::new(200.0, 300.0);
        interaction.mode = InteractionMode::Panning {
            drag_start: Vec2::new(100.0, 300.0),
        };
    }

    // Should not panic (division by zero protection)
    app.update();
}

#[test]
fn test_max_visible_candles_zoom_out() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    let candle_count = app.world().resource::<CandleData>().candles.len();

    // Set visible count to max
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.visible_candle_count = candle_count;
        viewport.visible_candle_start = 0;
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

    // Should be clamped to data length
    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert!(
        final_count <= candle_count,
        "Should not exceed candle count: {} <= {}",
        final_count,
        candle_count
    );
}

#[test]
fn test_min_visible_candles_zoom_in() {
    let mut app = create_test_app();
    app.add_systems(Update, handle_zoom);

    let min_candles = app
        .world()
        .resource::<charts::config::InteractionConfig>()
        .min_visible_candles as usize;

    // Set visible count to min
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

    // Should be clamped to min
    let final_count = app.world().resource::<ViewportState>().visible_candle_count;
    assert!(
        final_count >= min_candles,
        "Should not go below min: {} >= {}",
        final_count,
        min_candles
    );
}

// ============================================================================
// EDGE CASE TESTS - Gap Count
// ============================================================================

#[test]
fn test_two_panes_one_gap() {
    let app = create_test_app();

    let manager = app.world().resource::<PaneManager>();
    let num_gaps = manager.panes.len().saturating_sub(1);

    assert_eq!(num_gaps, 1, "Two panes should have one gap");
}

#[test]
fn test_single_pane_no_gaps() {
    let mut app = create_test_app();

    // Remove volume pane
    {
        let mut manager = app.world_mut().resource_mut::<PaneManager>();
        manager.panes.retain(|p| p.id == PaneId::Price);
    }

    let manager = app.world().resource::<PaneManager>();
    let num_gaps = manager.panes.len().saturating_sub(1);

    assert_eq!(num_gaps, 0, "Single pane should have no gaps");
}
