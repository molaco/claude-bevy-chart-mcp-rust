use bevy::prelude::*;
use crate::components::*;

/// System to display chat messages in the message container
pub fn update_message_display(
    mut commands: Commands,
    chat_state: Res<ChatState>,
    message_container: Query<(Entity, Option<&Children>), With<MessageContainer>>,
    existing_messages: Query<&ChatMessageEntity>,
    loading_indicator: Query<(Entity, Option<&Children>), With<LoadingIndicator>>,
) {
    // Return early if chat_state hasn't changed
    if !chat_state.is_changed() {
        return;
    }

    // Get the single MessageContainer entity and optionally its children
    let (container_entity, children_opt) = match message_container.iter().next() {
        Some(result) => result,
        None => {
            return;
        }
    };

    // Count existing message entities by filtering children that contain ChatMessageEntity
    let existing_count: usize = if let Some(children) = children_opt {
        children
            .iter()
            .filter(|child| existing_messages.get(*child).is_ok())
            .count()
    } else {
        0
    };

    // If chat_state.messages.len() > existing_count, spawn new message entities
    if chat_state.messages.len() > existing_count {
        for (index, message) in chat_state.messages.iter().enumerate().skip(existing_count) {

            // Terminal-style prefix and colors
            let prefix = if message.is_user { "You: " } else { "Assistant: " };
            let text_color = Color::srgb(0.9, 0.9, 0.92);    // Light gray for text

            // Spawn as child of container_entity - simple text, no bubble!
            commands.entity(container_entity).with_children(|parent| {
                parent.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,  // Stack prefix and content vertically
                        margin: UiRect::bottom(Val::Px(12.0)),  // Increased spacing
                        padding: UiRect::left(Val::Px(6.0)),     // Left padding for visual grouping
                        ..default()
                    },
                    ChatMessageEntity { index },
                ))
                .with_children(|message_node| {
                    // Spawn prefix text with dimmed color
                    message_node.spawn((
                        Text::new(prefix),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.6, 0.6, 0.62)),  // Dimmed color for prefix
                    ));
                    // Spawn message content with regular color
                    message_node.spawn((
                        Text::new(message.content.clone()),
                        TextFont::from_font_size(16.0),
                        TextColor(text_color),  // Regular bright color for content
                    ));
                });
            });
        }
    }

    // Handle loading indicator
    if chat_state.is_waiting {
        // Add loading indicator if not already present
        if loading_indicator.is_empty() {
            commands.entity(container_entity).with_children(|parent| {
                parent.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,  // Consistent with message layout
                        margin: UiRect::bottom(Val::Px(12.0)),  // Increased spacing to match messages
                        padding: UiRect::left(Val::Px(6.0)),     // Left padding for visual grouping
                        ..default()
                    },
                    LoadingIndicator,
                ))
                .with_children(|loading_node| {
                    loading_node.spawn((
                        Text::new("Claude is thinking..."),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.6, 0.7, 0.9)),  // Soft blue color for distinction
                    ));
                });
            });
        }
    } else {
        // Remove loading indicator if present
        for (entity, children_opt) in loading_indicator.iter() {
            // First despawn all children (text nodes)
            if let Some(children) = children_opt {
                for child in children.iter() {
                    commands.entity(child).despawn();
                }
            }
            // Then despawn the loading indicator itself
            commands.entity(entity).despawn();
        }
    }
}

/// System to auto-scroll to bottom when new messages are added
pub fn auto_scroll_to_bottom(
    mut message_container: Query<
        (&mut ScrollPosition, &ComputedNode, &Children),
        (With<MessageContainer>, Changed<Children>),
    >,
    node_query: Query<&ComputedNode>,
) {
    for (mut scroll_pos, container_node, children) in message_container.iter_mut() {
        // Calculate total_height = sum of all children node heights
        let total_height: f32 = children
            .iter()
            .filter_map(|child| node_query.get(child).ok())
            .map(|node| node.size().y)
            .sum();

        // Get container_height from container_node
        let container_height = container_node.size().y;

        // If total_height > container_height, set scroll_pos.0.y
        if total_height > container_height {
            scroll_pos.0.y = total_height - container_height;
        }
    }
}

/// System to handle manual scrolling with mouse wheel and arrow keys
pub fn handle_scroll_input(
    mut mouse_wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut message_container: Query<
        (&mut ScrollPosition, &ComputedNode, &Children),
        With<MessageContainer>,
    >,
    node_query: Query<&ComputedNode>,
) {
    let Ok((mut scroll_pos, container_node, children)) = message_container.single_mut() else {
        return;
    };

    // Calculate total content height
    let total_height: f32 = children
        .iter()
        .filter_map(|child| node_query.get(child).ok())
        .map(|node| node.size().y)
        .sum();

    let container_height = container_node.size().y;
    let max_scroll = (total_height - container_height).max(0.0);

    // Handle mouse wheel events
    for event in mouse_wheel_events.read() {
        let scroll_amount = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * 20.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y,
        };

        // Update scroll position (negative scroll amount because scrolling down increases Y)
        scroll_pos.0.y = (scroll_pos.0.y - scroll_amount).clamp(0.0, max_scroll);
    }

    // Handle arrow key scrolling
    let arrow_scroll_speed = 20.0; // pixels per frame
    if keys.pressed(KeyCode::ArrowUp) {
        scroll_pos.0.y = (scroll_pos.0.y - arrow_scroll_speed).max(0.0);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        scroll_pos.0.y = (scroll_pos.0.y + arrow_scroll_speed).min(max_scroll);
    }
}
