//! Integration tests for application startup and resource initialization.
//!
//! These tests verify that all required resources are properly initialized
//! when the chart application starts up.

mod common;

use charts::config::InteractionConfig;
use charts::coordinate::ViewportState;
use charts::data::CandleData;
use charts::indicators::IndicatorState;
use charts::interaction::InteractionState;
use charts::panes::PaneManager;

use common::{create_test_app, create_test_app_with_candles};

// ============================================================================
// RESOURCE EXISTENCE TESTS
// ============================================================================

#[test]
fn test_startup_all_resources_exist() {
    let app = create_test_app();

    // Verify all required resources exist
    assert!(
        app.world().get_resource::<CandleData>().is_some(),
        "CandleData should exist"
    );
    assert!(
        app.world().get_resource::<ViewportState>().is_some(),
        "ViewportState should exist"
    );
    assert!(
        app.world().get_resource::<PaneManager>().is_some(),
        "PaneManager should exist"
    );
    assert!(
        app.world().get_resource::<IndicatorState>().is_some(),
        "IndicatorState should exist"
    );
    assert!(
        app.world().get_resource::<InteractionState>().is_some(),
        "InteractionState should exist"
    );
    assert!(
        app.world().get_resource::<InteractionConfig>().is_some(),
        "InteractionConfig should exist"
    );
}

// ============================================================================
// VIEWPORT INITIALIZATION TESTS
// ============================================================================

#[test]
fn test_startup_pane_viewports_valid() {
    let app = create_test_app();

    let manager = app.world().resource::<PaneManager>();

    // All panes should have non-zero viewport dimensions
    for (i, pane) in manager.panes.iter().enumerate() {
        assert!(
            pane.space.viewport.width() > 0.0,
            "Pane {} should have non-zero width",
            i
        );
        assert!(
            pane.space.viewport.height() > 0.0,
            "Pane {} should have non-zero height",
            i
        );
    }
}

#[test]
fn test_startup_viewport_state_valid() {
    let app = create_test_app();

    let viewport = app.world().resource::<ViewportState>();

    assert!(
        viewport.visible_candle_count > 0,
        "Should have positive visible candle count"
    );
    assert!(
        viewport.total_area.width() > 0.0,
        "Total area should have width"
    );
    assert!(
        viewport.total_area.height() > 0.0,
        "Total area should have height"
    );
}

// ============================================================================
// DATA INITIALIZATION TESTS
// ============================================================================

#[test]
fn test_startup_candle_data_loaded() {
    let app = create_test_app();

    let candle_data = app.world().resource::<CandleData>();

    // Our test app creates 500 candles
    assert!(candle_data.candles.len() > 0, "Candle data should be loaded");
    assert_eq!(
        candle_data.candles.len(),
        500,
        "Test app should have 500 candles"
    );
}

#[test]
fn test_startup_candle_data_custom_count() {
    let app = create_test_app_with_candles(100);

    let candle_data = app.world().resource::<CandleData>();
    assert_eq!(candle_data.candles.len(), 100, "Should have 100 candles");
}

#[test]
fn test_startup_candle_data_valid_ohlcv() {
    let app = create_test_app();

    let candle_data = app.world().resource::<CandleData>();

    for (i, candle) in candle_data.candles.iter().enumerate() {
        assert!(
            candle.high >= candle.low,
            "Candle {} high should be >= low",
            i
        );
        assert!(
            candle.high >= candle.open && candle.high >= candle.close,
            "Candle {} high should be >= open and close",
            i
        );
        assert!(
            candle.low <= candle.open && candle.low <= candle.close,
            "Candle {} low should be <= open and close",
            i
        );
        assert!(candle.volume >= 0.0, "Candle {} volume should be >= 0", i);
    }
}

// ============================================================================
// PANE MANAGER INITIALIZATION TESTS
// ============================================================================

#[test]
fn test_startup_pane_manager_has_panes() {
    let app = create_test_app();

    let manager = app.world().resource::<PaneManager>();

    assert!(!manager.panes.is_empty(), "Should have at least one pane");
    assert_eq!(manager.panes.len(), 2, "Test app should have 2 panes");
}

#[test]
fn test_startup_pane_heights_sum_to_one() {
    let app = create_test_app();

    let manager = app.world().resource::<PaneManager>();

    let total_height: f32 = manager.panes.iter().map(|p| p.height_percent).sum();

    assert!(
        (total_height - 1.0).abs() < 0.01,
        "Pane heights should sum to ~1.0, got {}",
        total_height
    );
}

#[test]
fn test_startup_panes_non_overlapping() {
    let app = create_test_app();

    let manager = app.world().resource::<PaneManager>();

    // Check that pane viewports don't overlap
    for i in 0..manager.panes.len() {
        for j in (i + 1)..manager.panes.len() {
            let pane_i = &manager.panes[i];
            let pane_j = &manager.panes[j];

            // Simple check: viewport max Y of one should be <= min Y of other (or vice versa)
            let i_above_j = pane_i.space.viewport.min.y >= pane_j.space.viewport.max.y;
            let j_above_i = pane_j.space.viewport.min.y >= pane_i.space.viewport.max.y;

            assert!(
                i_above_j || j_above_i,
                "Panes {} and {} should not overlap vertically",
                i,
                j
            );
        }
    }
}

// ============================================================================
// INTERACTION STATE INITIALIZATION TESTS
// ============================================================================

#[test]
fn test_startup_interaction_state_idle() {
    let app = create_test_app();

    let interaction = app.world().resource::<InteractionState>();

    assert!(
        !interaction.mode.is_active(),
        "Initial interaction mode should not be active"
    );
}

// ============================================================================
// CONFIG INITIALIZATION TESTS
// ============================================================================

#[test]
fn test_startup_interaction_config_valid() {
    let app = create_test_app();

    let config = app.world().resource::<InteractionConfig>();

    // Zoom factors should be valid
    assert!(
        config.zoom_in_factor > 0.0 && config.zoom_in_factor < 1.0,
        "Zoom in factor should be (0, 1)"
    );
    assert!(
        config.zoom_out_factor > 1.0,
        "Zoom out factor should be > 1"
    );

    // Candle limits should be valid
    assert!(
        config.min_visible_candles > 0.0,
        "Min visible candles should be > 0"
    );
    assert!(
        config.max_visible_candles > config.min_visible_candles,
        "Max should be > min visible candles"
    );

    // Pane height constraints should be valid
    assert!(
        config.min_pane_height > 0.0 && config.min_pane_height < 0.5,
        "Min pane height should be (0, 0.5)"
    );
    assert!(
        config.max_pane_height > 0.5 && config.max_pane_height <= 1.0,
        "Max pane height should be (0.5, 1.0]"
    );
}
