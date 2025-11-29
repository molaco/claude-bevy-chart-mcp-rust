//! PaneManager - Multi-pane layout management resource.

use bevy::prelude::{Rect, Resource, Vec2};
use crate::domain::Candle;
use crate::indicators::MovingAverage;
use super::pane::{Pane, PaneId, PaneType};

/// Pane manager resource - manages multi-pane layout.
///
/// This resource holds all panes and provides methods for:
/// - Finding panes by ID
/// - Calculating layout positions based on height percentages
/// - Updating Y-axis bounds based on visible data
#[derive(Resource)]
pub struct PaneManager {
    /// All panes in the chart
    pub panes: Vec<Pane>,
    /// Gap between panes in pixels
    pub separator_gap: f32,
}

impl Default for PaneManager {
    fn default() -> Self {
        Self {
            panes: Vec::new(),
            separator_gap: crate::config::PANE_SEPARATOR_GAP,
        }
    }
}

impl PaneManager {
    /// Create a new PaneManager with the given panes.
    pub fn new(panes: Vec<Pane>) -> Self {
        Self {
            panes,
            separator_gap: crate::config::PANE_SEPARATOR_GAP,
        }
    }

    /// Create a new PaneManager with custom separator gap.
    pub fn new_with_gap(panes: Vec<Pane>, separator_gap: f32) -> Self {
        Self {
            panes,
            separator_gap,
        }
    }

    /// Find a pane by its ID.
    pub fn find_pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Find a mutable reference to a pane by its ID.
    pub fn find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// Get the price pane.
    pub fn price_pane(&self) -> Option<&Pane> {
        self.find_pane(PaneId::Price)
    }

    /// Get the volume pane.
    pub fn volume_pane(&self) -> Option<&Pane> {
        self.find_pane(PaneId::Volume)
    }

    /// Calculate and assign viewports to each pane based on height percentages.
    ///
    /// Panes are laid out from top to bottom, with separator gaps between them.
    pub fn calculate_layouts(&mut self, total_area: Rect, visible_candle_count: usize) {
        if self.panes.is_empty() {
            return;
        }

        // Calculate total height available after accounting for separators
        let num_separators = self.panes.len().saturating_sub(1);
        let separator_total_height = num_separators as f32 * self.separator_gap;
        let available_height = total_area.height() - separator_total_height;

        // Start from the top
        let mut current_y = total_area.max.y;
        let num_panes = self.panes.len();

        for (i, pane) in self.panes.iter_mut().enumerate() {
            let pane_height = available_height * pane.height_percent;
            let pane_min_y = current_y - pane_height;
            let pane_max_y = current_y;

            // Create viewport for this pane
            pane.space.viewport = Rect::from_corners(
                Vec2::new(total_area.min.x, pane_min_y),
                Vec2::new(total_area.max.x, pane_max_y),
            );

            // Recalculate cached values
            pane.space.recalculate_cache(visible_candle_count);

            // Move down for next pane, adding separator gap if not the last pane
            current_y = pane_min_y;
            if i < num_panes - 1 {
                current_y -= self.separator_gap;
            }
        }
    }

    /// Update Y-axis bounds for all panes based on their type.
    ///
    /// Automatically fits price bounds for price panes (including indicators)
    /// and volume bounds for volume panes.
    pub fn update_pane_bounds(
        &mut self,
        candles: &[Candle],
        indicators: &[MovingAverage],
        visible_start: usize,
        visible_count: usize,
    ) {
        for pane in self.panes.iter_mut() {
            match pane.pane_type {
                PaneType::Price => {
                    pane.space.fit_price_bounds_with_indicators(
                        candles,
                        indicators,
                        visible_start,
                        visible_count,
                    );
                }
                PaneType::Volume => {
                    pane.space
                        .fit_volume_bounds(candles, visible_start, visible_count);
                }
                PaneType::Indicator { .. } => {
                    // TODO: Handle indicator panes when implemented
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec2;

    fn create_test_pane(id: PaneId, pane_type: PaneType, height: f32) -> Pane {
        Pane::new(
            id,
            pane_type,
            height,
            Rect::default(),
            50,
        )
    }

    fn create_standard_two_pane_manager() -> PaneManager {
        let panes = vec![
            create_test_pane(PaneId::Price, PaneType::Price, 0.7),
            create_test_pane(PaneId::Volume, PaneType::Volume, 0.3),
        ];
        PaneManager::new(panes)
    }

    // ========================================================================
    // CREATION TESTS
    // ========================================================================

    #[test]
    fn test_pane_manager_default() {
        let manager = PaneManager::default();
        assert!(manager.panes.is_empty());
        assert_eq!(manager.separator_gap, crate::config::PANE_SEPARATOR_GAP);
    }

    #[test]
    fn test_pane_manager_new() {
        let manager = create_standard_two_pane_manager();
        assert_eq!(manager.panes.len(), 2);
        assert_eq!(manager.separator_gap, crate::config::PANE_SEPARATOR_GAP);
    }

    #[test]
    fn test_pane_manager_new_with_gap() {
        let panes = vec![
            create_test_pane(PaneId::Price, PaneType::Price, 0.7),
        ];
        let manager = PaneManager::new_with_gap(panes, 10.0);
        assert_eq!(manager.separator_gap, 10.0);
    }

    // ========================================================================
    // FIND PANE TESTS
    // ========================================================================

    #[test]
    fn test_find_pane_by_id() {
        let manager = create_standard_two_pane_manager();

        let price_pane = manager.find_pane(PaneId::Price);
        assert!(price_pane.is_some());
        assert_eq!(price_pane.unwrap().id, PaneId::Price);

        let volume_pane = manager.find_pane(PaneId::Volume);
        assert!(volume_pane.is_some());
        assert_eq!(volume_pane.unwrap().id, PaneId::Volume);
    }

    #[test]
    fn test_find_pane_not_found() {
        let manager = create_standard_two_pane_manager();

        let indicator_pane = manager.find_pane(PaneId::Indicator(0));
        assert!(indicator_pane.is_none());
    }

    #[test]
    fn test_find_pane_mut() {
        let mut manager = create_standard_two_pane_manager();

        if let Some(pane) = manager.find_pane_mut(PaneId::Price) {
            pane.height_percent = 0.8;
        }

        assert_eq!(manager.find_pane(PaneId::Price).unwrap().height_percent, 0.8);
    }

    #[test]
    fn test_price_pane_shortcut() {
        let manager = create_standard_two_pane_manager();
        let price_pane = manager.price_pane();
        assert!(price_pane.is_some());
        assert!(matches!(price_pane.unwrap().pane_type, PaneType::Price));
    }

    #[test]
    fn test_volume_pane_shortcut() {
        let manager = create_standard_two_pane_manager();
        let volume_pane = manager.volume_pane();
        assert!(volume_pane.is_some());
        assert!(matches!(volume_pane.unwrap().pane_type, PaneType::Volume));
    }

    // ========================================================================
    // CALCULATE LAYOUTS TESTS
    // ========================================================================

    #[test]
    fn test_calculate_layouts_two_panes() {
        let mut manager = create_standard_two_pane_manager();
        let total_area = Rect::from_corners(
            Vec2::new(0.0, 0.0),
            Vec2::new(800.0, 600.0),
        );

        manager.calculate_layouts(total_area, 50);

        // Check that viewports are assigned
        assert!(manager.panes[0].space.viewport.width() > 0.0);
        assert!(manager.panes[1].space.viewport.width() > 0.0);

        // Both should have same width as total area
        assert_eq!(manager.panes[0].space.viewport.width(), 800.0);
        assert_eq!(manager.panes[1].space.viewport.width(), 800.0);
    }

    #[test]
    fn test_calculate_layouts_separator_gap_subtracted() {
        let mut manager = PaneManager::new_with_gap(
            vec![
                create_test_pane(PaneId::Price, PaneType::Price, 0.5),
                create_test_pane(PaneId::Volume, PaneType::Volume, 0.5),
            ],
            20.0, // 20px gap
        );

        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        manager.calculate_layouts(total_area, 50);

        // Available height = 600 - 20 = 580
        // Each pane should have 290px height (50% of 580)
        let pane0_height = manager.panes[0].space.viewport.height();
        let pane1_height = manager.panes[1].space.viewport.height();

        assert!(
            (pane0_height - 290.0).abs() < 0.01,
            "Price pane height should be ~290px, got {}", pane0_height
        );
        assert!(
            (pane1_height - 290.0).abs() < 0.01,
            "Volume pane height should be ~290px, got {}", pane1_height
        );
    }

    #[test]
    fn test_calculate_layouts_heights_sum_correctly() {
        let mut manager = create_standard_two_pane_manager();
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

        manager.calculate_layouts(total_area, 50);

        let pane0_height = manager.panes[0].space.viewport.height();
        let pane1_height = manager.panes[1].space.viewport.height();
        let gap = manager.separator_gap;

        let total_used = pane0_height + pane1_height + gap;

        assert!(
            (total_used - 600.0).abs() < 0.01,
            "Total heights + gap should equal total area height: {} vs 600", total_used
        );
    }

    #[test]
    fn test_calculate_layouts_panes_non_overlapping() {
        let mut manager = create_standard_two_pane_manager();
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

        manager.calculate_layouts(total_area, 50);

        let pane0_bottom = manager.panes[0].space.viewport.min.y;
        let pane1_top = manager.panes[1].space.viewport.max.y;

        // The gap should be between them
        let actual_gap = pane0_bottom - pane1_top;

        assert!(
            (actual_gap - manager.separator_gap).abs() < 0.01,
            "Gap between panes should be separator_gap: {} vs {}", actual_gap, manager.separator_gap
        );
    }

    #[test]
    fn test_calculate_layouts_panes_top_to_bottom() {
        let mut manager = create_standard_two_pane_manager();
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

        manager.calculate_layouts(total_area, 50);

        // Price pane (first) should be higher (larger Y) than volume pane
        let pane0_top = manager.panes[0].space.viewport.max.y;
        let pane1_top = manager.panes[1].space.viewport.max.y;

        assert!(
            pane0_top > pane1_top,
            "First pane should be above second pane: {} > {}", pane0_top, pane1_top
        );
    }

    #[test]
    fn test_calculate_layouts_empty_panes() {
        let mut manager = PaneManager::default();
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

        // Should not panic
        manager.calculate_layouts(total_area, 50);
    }

    #[test]
    fn test_calculate_layouts_single_pane() {
        let mut manager = PaneManager::new(vec![
            create_test_pane(PaneId::Price, PaneType::Price, 1.0),
        ]);
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

        manager.calculate_layouts(total_area, 50);

        // Single pane should take full height (no separators)
        let pane_height = manager.panes[0].space.viewport.height();
        assert!(
            (pane_height - 600.0).abs() < 0.01,
            "Single pane should take full height"
        );
    }

    #[test]
    fn test_calculate_layouts_three_panes() {
        let mut manager = PaneManager::new_with_gap(
            vec![
                create_test_pane(PaneId::Price, PaneType::Price, 0.5),
                create_test_pane(PaneId::Volume, PaneType::Volume, 0.25),
                create_test_pane(PaneId::Indicator(0), PaneType::Indicator { name: "RSI".to_string() }, 0.25),
            ],
            10.0,
        );
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

        manager.calculate_layouts(total_area, 50);

        // 2 gaps between 3 panes = 20px total
        // Available = 600 - 20 = 580
        let expected_price_height = 580.0 * 0.5;  // 290
        let expected_volume_height = 580.0 * 0.25; // 145

        assert!(
            (manager.panes[0].space.viewport.height() - expected_price_height).abs() < 0.01
        );
        assert!(
            (manager.panes[1].space.viewport.height() - expected_volume_height).abs() < 0.01
        );
    }

    // ========================================================================
    // UPDATE PANE BOUNDS TESTS
    // ========================================================================

    #[test]
    fn test_update_pane_bounds_sets_price_range() {
        let mut manager = create_standard_two_pane_manager();
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        manager.calculate_layouts(total_area, 50);

        let candles = vec![
            Candle::new(1000, 100.0, 110.0, 95.0, 105.0, 500.0),
            Candle::new(2000, 105.0, 120.0, 100.0, 115.0, 1000.0),
        ];

        manager.update_pane_bounds(&candles, &[], 0, 2);

        let price_pane = manager.price_pane().unwrap();
        // Price range should be set based on candle highs/lows
        assert!(price_pane.space.visible_price_min < 100.0, "Should include low with padding");
        assert!(price_pane.space.visible_price_max > 115.0, "Should include high with padding");
    }

    #[test]
    fn test_update_pane_bounds_sets_volume_range() {
        let mut manager = create_standard_two_pane_manager();
        let total_area = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        manager.calculate_layouts(total_area, 50);

        let candles = vec![
            Candle::new(1000, 100.0, 110.0, 95.0, 105.0, 500.0),
            Candle::new(2000, 105.0, 120.0, 100.0, 115.0, 1000.0),
        ];

        manager.update_pane_bounds(&candles, &[], 0, 2);

        let volume_pane = manager.volume_pane().unwrap();
        assert_eq!(volume_pane.space.visible_price_min, 0.0, "Volume min should be 0");
        assert_eq!(volume_pane.space.visible_price_max, 1000.0, "Volume max should be highest volume");
    }
}
