//! Shared test utilities for integration tests.
//!
//! This module provides common helper functions for creating test fixtures
//! and setting up Bevy apps for testing chart systems.

#![allow(deprecated)] // Allow deprecated Bevy 0.17 event APIs

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;

use charts::config::InteractionConfig;
use charts::coordinate::ViewportState;
use charts::data::CandleData;
use charts::domain::Candle;
use charts::indicators::IndicatorState;
use charts::interaction::InteractionState;
use charts::panes::{Pane, PaneId, PaneManager, PaneType};

// ============================================================================
// CANDLE DATA HELPERS
// ============================================================================

/// Create test candles with predictable OHLCV values.
///
/// Each candle has incrementing prices:
/// - time: i * 60000 (one minute intervals)
/// - open: 100.0 + i
/// - high: 110.0 + i
/// - low: 90.0 + i
/// - close: 105.0 + i
/// - volume: 1000.0 + i * 10
pub fn create_test_candles(count: usize) -> Vec<Candle> {
    (0..count)
        .map(|i| {
            Candle::new(
                (i as i64 + 1) * 60000, // time: every minute
                100.0 + i as f64,       // open
                110.0 + i as f64,       // high
                90.0 + i as f64,        // low
                105.0 + i as f64,       // close
                1000.0 + i as f64 * 10.0, // volume
            )
        })
        .collect()
}

/// Create candles with a specific price range.
pub fn create_candles_in_range(count: usize, min_price: f64, max_price: f64) -> Vec<Candle> {
    let price_step = (max_price - min_price) / count.max(1) as f64;
    (0..count)
        .map(|i| {
            let base_price = min_price + i as f64 * price_step;
            Candle::new(
                (i as i64 + 1) * 60000,
                base_price,
                base_price + 5.0,
                base_price - 5.0,
                base_price + 2.0,
                1000.0,
            )
        })
        .collect()
}

// ============================================================================
// RESOURCE HELPERS
// ============================================================================

/// Standard test viewport area (800x600).
pub fn test_viewport_area() -> Rect {
    Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0))
}

/// Create a standard two-pane manager (Price 70%, Volume 30%).
pub fn create_test_pane_manager() -> PaneManager {
    let total_area = test_viewport_area();
    let mut manager = PaneManager::new_with_gap(
        vec![
            Pane::new(PaneId::Price, PaneType::Price, 0.7, total_area, 100),
            Pane::new(PaneId::Volume, PaneType::Volume, 0.3, total_area, 100),
        ],
        10.0,
    );
    manager.calculate_layouts(total_area, 100);
    manager
}

/// Create a single-pane manager (Price only, 100%).
pub fn create_single_pane_manager() -> PaneManager {
    let total_area = test_viewport_area();
    let mut manager = PaneManager::new_with_gap(
        vec![
            Pane::new(PaneId::Price, PaneType::Price, 1.0, total_area, 100),
        ],
        10.0,
    );
    manager.calculate_layouts(total_area, 100);
    manager
}

/// Create a ViewportState with standard values.
///
/// - visible_candle_start: 50
/// - visible_candle_count: 100
/// - total_area: 800x600
pub fn create_test_viewport() -> ViewportState {
    ViewportState::new(
        50,   // start at candle 50
        100,  // show 100 candles
        test_viewport_area(),
    )
}

// ============================================================================
// BEVY APP HELPERS
// ============================================================================

/// Create a minimal Bevy App with chart resources for testing.
///
/// Includes:
/// - MinimalPlugins (time, events)
/// - MouseWheel event type
/// - CandleData with 500 test candles
/// - ViewportState (start=50, count=100)
/// - PaneManager (2 panes: Price 70%, Volume 30%)
/// - IndicatorState (empty)
/// - InteractionState (Idle)
/// - InteractionConfig (defaults)
pub fn create_test_app() -> App {
    let mut app = App::new();

    // Add MinimalPlugins for basic Bevy functionality (time, events)
    app.add_plugins(MinimalPlugins);

    // Add MouseWheel event type
    app.add_event::<MouseWheel>();

    // Add chart resources
    let candles = create_test_candles(500);
    app.insert_resource(CandleData::new(candles));
    app.insert_resource(create_test_viewport());
    app.insert_resource(create_test_pane_manager());
    app.insert_resource(IndicatorState::default());
    app.insert_resource(InteractionState::default());
    app.insert_resource(InteractionConfig::default());

    app
}

/// Create a test app with custom candle count.
pub fn create_test_app_with_candles(candle_count: usize) -> App {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins);
    app.add_event::<MouseWheel>();

    let candles = create_test_candles(candle_count);
    app.insert_resource(CandleData::new(candles));
    app.insert_resource(create_test_viewport());
    app.insert_resource(create_test_pane_manager());
    app.insert_resource(IndicatorState::default());
    app.insert_resource(InteractionState::default());
    app.insert_resource(InteractionConfig::default());

    app
}

// ============================================================================
// ASSERTION HELPERS
// ============================================================================

/// Assert two floats are approximately equal.
pub fn assert_float_eq(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() < tolerance,
        "Float mismatch: {} vs {} (tolerance {})",
        actual, expected, tolerance
    );
}

/// Assert two Vec2s are approximately equal.
pub fn assert_vec2_eq(actual: Vec2, expected: Vec2, tolerance: f32) {
    assert!(
        (actual.x - expected.x).abs() < tolerance && (actual.y - expected.y).abs() < tolerance,
        "Vec2 mismatch: {:?} vs {:?} (tolerance {})",
        actual, expected, tolerance
    );
}
