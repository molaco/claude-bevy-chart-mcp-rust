use bevy::prelude::*;

/// Resource that holds the global chat state
#[derive(Resource, Default)]
pub struct ChatState {
    /// All chat messages in chronological order
    pub messages: Vec<ChatMessage>,
    /// Whether we're currently waiting for a Claude response
    pub is_waiting: bool,
}

/// Represents a single chat message
#[derive(Clone)]
pub struct ChatMessage {
    /// The message text content
    pub content: String,
    /// True if this message is from the user, false if from the assistant
    pub is_user: bool,
    /// When this message was sent
    pub timestamp: std::time::SystemTime,
}

/// Marker component for the text input field entity
#[derive(Component)]
pub struct ChatInputField;

/// Marker component for the message list container entity
#[derive(Component)]
pub struct MessageContainer;

/// Marker component for individual message entities
#[derive(Component)]
pub struct ChatMessageEntity {
    /// Index of this message in the ChatState.messages vector
    pub index: usize,
}

/// Marker component for the loading indicator ("thinking..." text)
#[derive(Component)]
pub struct LoadingIndicator;
