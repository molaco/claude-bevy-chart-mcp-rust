//! Complete chat interface example with real Claude responses

use bevy::prelude::*;
use chat_ui::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ChatUiPlugin)
        .run();
}

// No mock responses needed - the ChatUiPlugin now uses real Claude!
