use bevy::prelude::*;

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
                    ..default()
                },
                ChartViewport,
            ));

            // Right side: Chat UI container (30%)
            parent.spawn((
                Node {
                    width: Val::Percent(30.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ChatUiContainer,
            ));
        });
}
