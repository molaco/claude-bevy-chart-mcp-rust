use crate::focus::{ChartFocusIndicator, ChatFocusIndicator};
use bevy::prelude::*;
use chat_ui::prelude::ChatUiRoot;

/// Marker component for the chart viewport area (left 70%)
#[derive(Component)]
pub struct ChartViewport;

/// Marker component for the chat UI area (right 30%)
#[derive(Component)]
pub struct ChatUiContainer;

/// System to set up the split layout with chart on left and chat on right
pub fn setup_split_layout(mut commands: Commands) {
    // Root container - horizontal split
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|parent| {
            // Left side: Chart viewport container (70%)
            parent
                .spawn((
                    Node {
                        width: Val::Percent(70.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center, // Center chart horizontally
                        align_items: AlignItems::Center,         // Center chart vertically
                        ..default()
                    },
                    ChartViewport,
                ))
                .with_children(|viewport| {
                    // Focus indicator border for chart (visible when chart has focus)
                    viewport.spawn((
                        Node {
                            width: Val::Percent(90.0),  // Slightly smaller than container
                            height: Val::Percent(70.0), // Leave margins
                            border: UiRect::all(Val::Px(3.0)),
                            display: Display::Flex, // Visible by default (chart starts with focus)
                            ..default()
                        },
                        // BorderColor::all(Color::srgb(0.2, 0.6, 1.0)), // Blue focus border
                        BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 0.6)),
                        BackgroundColor(Color::NONE), // Transparent background
                        ChartFocusIndicator,
                    ));
                });

            // Right side: Chat UI container (30%)
            parent
                .spawn((
                    Node {
                        width: Val::Percent(30.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center, // Center chat vertically
                        justify_content: JustifyContent::FlexStart, // aligns to left
                        ..default()
                    },
                    ChatUiContainer,
                ))
                .with_children(|container| {
                    // Focus indicator border for chat (hidden by default)
                    container.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect::all(Val::Px(3.0)),
                            display: Display::None, // Hidden by default
                            ..default()
                        },
                        // BorderColor::all(Color::srgb(0.2, 0.6, 1.0)), // Blue focus border
                        BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 0.6)),
                        BackgroundColor(Color::NONE), // Transparent background
                        ChatFocusIndicator,
                    ));
                });
        });
}

/// System to reparent chat UI into the ChatUiContainer
/// This runs in PostStartup after all Startup systems complete
pub fn reparent_chat_to_container(
    mut commands: Commands,
    chat_root: Query<Entity, With<ChatUiRoot>>,
    container: Query<Entity, With<ChatUiContainer>>,
) {
    match (chat_root.single(), container.single()) {
        (Ok(chat_entity), Ok(container_entity)) => {
            println!(
                "✓ Reparenting chat (entity {:?}) to container (entity {:?})",
                chat_entity, container_entity
            );
            commands.entity(container_entity).add_child(chat_entity);
        }
        (Err(e1), Err(e2)) => {
            eprintln!(
                "✗ Reparenting failed: chat query error: {:?}, container query error: {:?}",
                e1, e2
            );
        }
        (Err(e), Ok(_)) => {
            eprintln!("✗ Reparenting failed: chat not found: {:?}", e);
        }
        (Ok(_), Err(e)) => {
            eprintln!("✗ Reparenting failed: container not found: {:?}", e);
        }
    }
}
