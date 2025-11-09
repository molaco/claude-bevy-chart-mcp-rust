use bevy::prelude::*;
use bevy_ui_text_input::SubmitText;
use crate::components::*;

pub fn handle_text_submit(
    mut submit_events: MessageReader<SubmitText>,
    mut chat_state: ResMut<ChatState>,
    input_query: Query<Entity, With<ChatInputField>>,
) {
    for event in submit_events.read() {
        // Check if the event entity matches our ChatInputField
        if let Ok(_entity) = input_query.get(event.entity) {
            // Get the text content from event
            let text = event.text.trim();

            // Skip if the text is empty (after trim)
            if text.is_empty() {
                continue;
            }

            // Add a new ChatMessage to chat_state.messages
            chat_state.messages.push(ChatMessage {
                content: text.to_string(),
                is_user: true,
                timestamp: std::time::SystemTime::now(),
            });

            // Set chat_state.is_waiting = true
            chat_state.is_waiting = true;
        }
    }
}
