use bevy::prelude::*;
use bevy::input_focus::InputFocus;
use chat_ui::prelude::ChatInputField;

// ============================================================================
// FOCUS MANAGEMENT
// ============================================================================

/// Represents which component currently has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Chart,
    Chat,
}

/// Resource tracking the current focus state
#[derive(Resource, Default)]
pub struct FocusState {
    pub current: FocusTarget,
    /// Window boundaries for hit testing (updated on window resize)
    pub chart_boundary: f32, // X coordinate separating chart from chat (70% of window width)
}

impl Default for FocusTarget {
    fn default() -> Self {
        FocusTarget::Chart
    }
}

/// Marker component for the chart focus indicator (border)
#[derive(Component)]
pub struct ChartFocusIndicator;

/// Marker component for the chat focus indicator (border)
#[derive(Component)]
pub struct ChatFocusIndicator;

// ============================================================================
// FOCUS SWITCHING SYSTEMS
// ============================================================================

/// System to handle Tab key for focus switching
pub fn handle_focus_tab(
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<FocusState>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        focus.current = match focus.current {
            FocusTarget::Chart => FocusTarget::Chat,
            FocusTarget::Chat => FocusTarget::Chart,
        };
        println!("Focus switched to: {:?}", focus.current);
    }
}

/// System to handle mouse click focus switching
pub fn handle_focus_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    mut focus: ResMut<FocusState>,
) {
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Ok(window) = window_query.single() {
            if let Some(cursor_pos) = window.cursor_position() {
                // Update chart boundary based on current window size
                let chart_width = window.width() * 0.7;
                focus.chart_boundary = chart_width;

                // Determine which side was clicked
                let new_focus = if cursor_pos.x <= chart_width {
                    FocusTarget::Chart
                } else {
                    FocusTarget::Chat
                };

                if new_focus != focus.current {
                    focus.current = new_focus;
                    println!("Focus switched to: {:?}", focus.current);
                }
            }
        }
    }
}

/// System to update focus indicator visibility
pub fn update_focus_indicators(
    focus: Res<FocusState>,
    mut chart_indicator_query: Query<&mut Node, (With<ChartFocusIndicator>, Without<ChatFocusIndicator>)>,
    mut chat_indicator_query: Query<&mut Node, (With<ChatFocusIndicator>, Without<ChartFocusIndicator>)>,
) {
    // Update chart indicator
    if let Ok(mut node) = chart_indicator_query.single_mut() {
        node.display = if focus.current == FocusTarget::Chart {
            Display::Flex
        } else {
            Display::None
        };
    }

    // Update chat indicator
    if let Ok(mut node) = chat_indicator_query.single_mut() {
        node.display = if focus.current == FocusTarget::Chat {
            Display::Flex
        } else {
            Display::None
        };
    }
}

/// System to manage text input focus based on FocusState
pub fn manage_text_input_focus(
    focus: Res<FocusState>,
    input_query: Query<Entity, With<ChatInputField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    // Only update if focus state changed
    if !focus.is_changed() {
        return;
    }

    if let Ok(input_entity) = input_query.single() {
        match focus.current {
            FocusTarget::Chat => {
                // Give focus to the text input
                input_focus.0 = Some(input_entity);
            }
            FocusTarget::Chart => {
                // Remove focus from text input if it currently has it
                if input_focus.0 == Some(input_entity) {
                    input_focus.0 = None;
                }
            }
        }
    }
}
