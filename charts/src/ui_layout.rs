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
            parent.spawn((
                Node {
                    width: Val::Percent(70.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,  // Center chart horizontally
                    align_items: AlignItems::Center,          // Center chart vertically
                    ..default()
                },
                ChartViewport,
            ))
            .with_children(|viewport| {
                // Border frame to show chart boundaries
                viewport.spawn((
                    Node {
                        width: Val::Percent(95.0),   // Slightly smaller than container
                        height: Val::Percent(90.0),  // Leave margins
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.3, 0.3, 0.35, 0.8)),  // Semi-transparent border
                    BackgroundColor(Color::NONE),  // Transparent background
                ));
            });

            // Right side: Chat UI container (30%)
            parent.spawn((
                Node {
                    width: Val::Percent(30.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,  // Center chat vertically
                    ..default()
                },
                ChatUiContainer,
            ));
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
            println!("✓ Reparenting chat (entity {:?}) to container (entity {:?})", chat_entity, container_entity);
            commands.entity(container_entity).add_child(chat_entity);
        }
        (Err(e1), Err(e2)) => {
            eprintln!("✗ Reparenting failed: chat query error: {:?}, container query error: {:?}", e1, e2);
        }
        (Err(e), Ok(_)) => {
            eprintln!("✗ Reparenting failed: chat not found: {:?}", e);
        }
        (Ok(_), Err(e)) => {
            eprintln!("✗ Reparenting failed: container not found: {:?}", e);
        }
    }
}
