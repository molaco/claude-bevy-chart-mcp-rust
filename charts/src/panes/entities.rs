//! Persistent entity references for rendering components.
//!
//! This module contains resources that store entity references for components
//! that are created once during setup and updated efficiently during rendering
//! without despawning/respawning.

use bevy::prelude::{Entity, Resource};
use super::pane::PaneId;

/// Persistent crosshair entities (created once, updated every frame).
///
/// This resource stores entity references for the crosshair components,
/// allowing them to be created once during setup and updated efficiently
/// during rendering without despawning/respawning.
#[derive(Resource)]
pub struct CrosshairEntities {
    /// Multiple dashed segments for vertical crosshair line
    pub vertical_line_segments: Vec<Entity>,
    /// Dashed segments per pane for horizontal lines
    pub horizontal_lines: Vec<(PaneId, Vec<Entity>)>,
    /// One price label per pane
    pub price_labels: Vec<(PaneId, Entity)>,
    /// Time label at the bottom
    pub time_label: Entity,
    /// OHLCV info box
    pub ohlcv_box: Entity,
}

/// Persistent grid and border entities (created once, updated every frame).
///
/// This resource stores entity references for grid lines and pane borders,
/// eliminating entity churn by reusing entities instead of despawning/respawning.
#[derive(Resource, Default)]
pub struct GridBorderEntities {
    /// Horizontal grid lines per pane: (PaneId, [line entities for each tick])
    pub horizontal_grid_lines: Vec<(PaneId, Vec<Entity>)>,

    /// Vertical grid lines (shared across panes): entities for each x tick
    pub vertical_grid_lines: Vec<Entity>,

    /// Pane border entities: (PaneId, [top, bottom, left, right])
    pub pane_borders: Vec<(PaneId, [Entity; 4])>,

    /// Number of Y ticks configured (for detecting layout changes)
    pub y_tick_count: usize,

    /// Number of X ticks configured (for detecting layout changes)
    pub x_tick_count: usize,

    /// Whether entities have been initialized
    pub initialized: bool,
}
