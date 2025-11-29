//! CrosshairEntities - Persistent entity references for crosshair rendering.

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
