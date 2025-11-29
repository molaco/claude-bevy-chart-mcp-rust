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
