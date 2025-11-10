use bevy::prelude::*;

pub mod components;
pub mod layout;
pub mod input;
pub mod messages;
pub mod claude;

pub struct ChatUiPlugin;

impl Plugin for ChatUiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add the Tokio runtime plugin for async task support
            .add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default())
            .add_plugins(bevy_ui_text_input::TextInputPlugin)
            .init_resource::<components::ChatState>()
            .add_systems(Startup, (
                layout::setup_chat_ui,
                claude::init_claude_client,
            ))
            .add_systems(Update, (
                input::handle_text_submit,
                messages::update_message_display,
                messages::auto_scroll_to_bottom,
                claude::send_to_claude,
                claude::receive_from_claude,
            ));
    }
}

pub mod prelude {
    pub use crate::{
        ChatUiPlugin,
        components::*,
        layout::ChatUiRoot,
    };
}
