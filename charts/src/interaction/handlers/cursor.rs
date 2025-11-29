use bevy::prelude::*;
use bevy::window::{CursorIcon, SystemCursorIcon};

use crate::interaction::state::InteractionState;

// ============================================================================
// CURSOR HANDLER
// ============================================================================

/// Update cursor position from window coordinates to world space.
/// Must run first in the input phase to have correct mouse position
/// for all subsequent systems.
pub fn update_cursor_position(
    window_query: Query<&Window>,
    mut interaction: ResMut<InteractionState>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    // Convert cursor position from screen space to world space
    // Bevy's cursor_position() has (0,0) at top-left with Y down
    // World space has (0,0) at center with Y up - need to flip Y
    if let Some(cursor_pos) = window.cursor_position() {
        let window_size = Vec2::new(window.width(), window.height());
        interaction.mouse_pos = Vec2::new(
            cursor_pos.x - window_size.x / 2.0,
            window_size.y / 2.0 - cursor_pos.y, // Flip Y axis
        );
    }
}

/// Update cursor icon based on current interaction mode.
/// Changes cursor to resize icon when hovering/dragging gaps.
pub fn update_cursor_icon(
    mut commands: Commands,
    interaction: Res<InteractionState>,
    window_query: Query<Entity, With<Window>>,
) {
    let Ok(window_entity) = window_query.single() else {
        return;
    };

    let cursor = if interaction.should_show_resize_cursor() {
        CursorIcon::from(SystemCursorIcon::NsResize)
    } else {
        CursorIcon::from(SystemCursorIcon::Default)
    };

    commands.entity(window_entity).insert(cursor);
}
