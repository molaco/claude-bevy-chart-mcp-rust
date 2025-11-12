//! Minimal example - just the text input

use bevy::prelude::*;
use bevy_ui_text_input::{SubmitText, TextInputMode, TextInputNode, TextInputPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TextInputPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, handle_submit)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        TextInputNode {
            clear_on_submit: true,
            mode: TextInputMode::SingleLine,
            ..default()
        },
        Node {
            width: Val::Px(400.0),
            height: Val::Px(100.0),
            margin: UiRect::all(Val::Auto),
            ..default()
        },
        BackgroundColor(Color::NONE),
    ));
}

fn handle_submit(mut submit_events: MessageReader<SubmitText>) {
    for event in submit_events.read() {
        println!("Submitted text: {}", event.text);
    }
}
