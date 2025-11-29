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
