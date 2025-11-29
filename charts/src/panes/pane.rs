//! Pane types - Individual pane configuration and identification.

use bevy::prelude::{Component, Rect};
use crate::coordinate::ChartSpace;

/// Pane identifier.
///
/// Used to uniquely identify different panes in the chart layout.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PaneId {
    /// The main price pane showing candlesticks
    Price,
    /// The volume pane showing volume bars
    Volume,
    /// An indicator pane (indexed for multiple indicators)
    Indicator(usize),
}

/// What type of content this pane renders.
///
/// Determines how the pane calculates its Y-axis bounds and
/// what data it displays.
#[derive(Clone, Debug)]
pub enum PaneType {
    /// Price pane - shows candlesticks with price on Y-axis
    Price,
    /// Volume pane - shows volume bars with volume on Y-axis
    Volume,
    /// Indicator pane - shows indicator values
    Indicator { name: String },
}

/// Individual pane configuration.
///
/// Each pane has its own coordinate space, height percentage,
/// and content type. Panes are managed by [`PaneManager`].
#[derive(Clone)]
pub struct Pane {
    /// Unique identifier for this pane
    pub id: PaneId,
    /// Type of content this pane displays
    pub pane_type: PaneType,
    /// Height as a percentage of total available space (0.0 - 1.0)
    pub height_percent: f32,
    /// Coordinate space for this pane
    pub space: ChartSpace,
}

impl Pane {
    /// Create a new pane with the given configuration.
    pub fn new(
        id: PaneId,
        pane_type: PaneType,
        height_percent: f32,
        viewport: Rect,
        visible_candle_count: usize,
    ) -> Self {
        Self {
            id,
            pane_type,
            height_percent,
            space: ChartSpace::new(viewport, visible_candle_count),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec2;

    fn create_test_viewport() -> Rect {
        Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 400.0))
    }

    // ========================================================================
    // PaneId TESTS
    // ========================================================================

    #[test]
    fn test_pane_id_price() {
        let id = PaneId::Price;
        assert_eq!(id, PaneId::Price);
    }

    #[test]
    fn test_pane_id_volume() {
        let id = PaneId::Volume;
        assert_eq!(id, PaneId::Volume);
    }

    #[test]
    fn test_pane_id_indicator() {
        let id = PaneId::Indicator(0);
        assert_eq!(id, PaneId::Indicator(0));
        assert_ne!(id, PaneId::Indicator(1));
    }

    #[test]
    fn test_pane_id_equality() {
        assert_eq!(PaneId::Price, PaneId::Price);
        assert_eq!(PaneId::Volume, PaneId::Volume);
        assert_eq!(PaneId::Indicator(5), PaneId::Indicator(5));

        assert_ne!(PaneId::Price, PaneId::Volume);
        assert_ne!(PaneId::Indicator(0), PaneId::Indicator(1));
    }

    #[test]
    fn test_pane_id_clone() {
        let id = PaneId::Indicator(3);
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }

    #[test]
    fn test_pane_id_copy() {
        let id = PaneId::Price;
        let copied = id; // Copy trait
        assert_eq!(id, copied);
    }

    #[test]
    fn test_pane_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(PaneId::Price);
        set.insert(PaneId::Volume);
        set.insert(PaneId::Price); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&PaneId::Price));
        assert!(set.contains(&PaneId::Volume));
    }

    #[test]
    fn test_pane_id_debug() {
        let id = PaneId::Indicator(42);
        let debug_str = format!("{:?}", id);
        assert!(debug_str.contains("Indicator"));
        assert!(debug_str.contains("42"));
    }

    // ========================================================================
    // PaneType TESTS
    // ========================================================================

    #[test]
    fn test_pane_type_price() {
        let pt = PaneType::Price;
        assert!(matches!(pt, PaneType::Price));
    }

    #[test]
    fn test_pane_type_volume() {
        let pt = PaneType::Volume;
        assert!(matches!(pt, PaneType::Volume));
    }

    #[test]
    fn test_pane_type_indicator() {
        let pt = PaneType::Indicator { name: "RSI".to_string() };
        match pt {
            PaneType::Indicator { name } => assert_eq!(name, "RSI"),
            _ => panic!("Expected Indicator type"),
        }
    }

    #[test]
    fn test_pane_type_clone() {
        let pt = PaneType::Indicator { name: "MACD".to_string() };
        let cloned = pt.clone();
        match cloned {
            PaneType::Indicator { name } => assert_eq!(name, "MACD"),
            _ => panic!("Clone failed"),
        }
    }

    #[test]
    fn test_pane_type_debug() {
        let pt = PaneType::Price;
        let debug_str = format!("{:?}", pt);
        assert!(debug_str.contains("Price"));
    }

    // ========================================================================
    // Pane TESTS
    // ========================================================================

    #[test]
    fn test_pane_new_price() {
        let viewport = create_test_viewport();
        let pane = Pane::new(PaneId::Price, PaneType::Price, 0.7, viewport, 100);

        assert_eq!(pane.id, PaneId::Price);
        assert!(matches!(pane.pane_type, PaneType::Price));
        assert_eq!(pane.height_percent, 0.7);
        assert_eq!(pane.space.viewport, viewport);
    }

    #[test]
    fn test_pane_new_volume() {
        let viewport = create_test_viewport();
        let pane = Pane::new(PaneId::Volume, PaneType::Volume, 0.3, viewport, 100);

        assert_eq!(pane.id, PaneId::Volume);
        assert!(matches!(pane.pane_type, PaneType::Volume));
        assert_eq!(pane.height_percent, 0.3);
    }

    #[test]
    fn test_pane_new_indicator() {
        let viewport = create_test_viewport();
        let pane = Pane::new(
            PaneId::Indicator(0),
            PaneType::Indicator { name: "Bollinger".to_string() },
            0.25,
            viewport,
            50,
        );

        assert_eq!(pane.id, PaneId::Indicator(0));
        match &pane.pane_type {
            PaneType::Indicator { name } => assert_eq!(name, "Bollinger"),
            _ => panic!("Expected Indicator type"),
        }
        assert_eq!(pane.height_percent, 0.25);
    }

    #[test]
    fn test_pane_chart_space_initialized() {
        let viewport = Rect::from_corners(Vec2::new(100.0, 50.0), Vec2::new(900.0, 450.0));
        let pane = Pane::new(PaneId::Price, PaneType::Price, 0.7, viewport, 200);

        assert_eq!(pane.space.viewport, viewport);
        // ChartSpace should be initialized with the candle count
        assert!(pane.space.candle_width_px > 0.0);
    }

    #[test]
    fn test_pane_clone() {
        let viewport = create_test_viewport();
        let pane = Pane::new(PaneId::Price, PaneType::Price, 0.7, viewport, 100);
        let cloned = pane.clone();

        assert_eq!(pane.id, cloned.id);
        assert_eq!(pane.height_percent, cloned.height_percent);
        assert_eq!(pane.space.viewport, cloned.space.viewport);
    }

    #[test]
    fn test_pane_height_percent_boundaries() {
        let viewport = create_test_viewport();

        // Min height
        let pane_min = Pane::new(PaneId::Price, PaneType::Price, 0.1, viewport, 100);
        assert_eq!(pane_min.height_percent, 0.1);

        // Max height
        let pane_max = Pane::new(PaneId::Price, PaneType::Price, 0.9, viewport, 100);
        assert_eq!(pane_max.height_percent, 0.9);

        // Full height
        let pane_full = Pane::new(PaneId::Price, PaneType::Price, 1.0, viewport, 100);
        assert_eq!(pane_full.height_percent, 1.0);
    }

    #[test]
    fn test_pane_with_different_visible_candle_counts() {
        let viewport = create_test_viewport();

        // Few candles
        let pane_few = Pane::new(PaneId::Price, PaneType::Price, 0.7, viewport, 10);
        assert!(pane_few.space.candle_width_px > 0.0);

        // Many candles
        let pane_many = Pane::new(PaneId::Price, PaneType::Price, 0.7, viewport, 1000);
        assert!(pane_many.space.candle_width_px > 0.0);

        // More candles = smaller candle width
        assert!(pane_few.space.candle_width_px > pane_many.space.candle_width_px);
    }

    #[test]
    fn test_pane_zero_viewport() {
        let viewport = Rect::from_corners(Vec2::ZERO, Vec2::ZERO);
        let pane = Pane::new(PaneId::Price, PaneType::Price, 0.7, viewport, 100);

        // Should not panic, but candle_width_px might be 0 or NaN
        let _ = pane.space.candle_width_px;
    }

    #[test]
    fn test_multiple_indicator_panes() {
        let viewport = create_test_viewport();

        let panes: Vec<Pane> = (0..5)
            .map(|i| {
                Pane::new(
                    PaneId::Indicator(i),
                    PaneType::Indicator { name: format!("Indicator-{}", i) },
                    0.1,
                    viewport,
                    100,
                )
            })
            .collect();

        assert_eq!(panes.len(), 5);

        for (i, pane) in panes.iter().enumerate() {
            assert_eq!(pane.id, PaneId::Indicator(i));
        }
    }
}
