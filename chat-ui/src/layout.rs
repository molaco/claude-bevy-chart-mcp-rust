use bevy::prelude::*;
use bevy::input_focus::AutoFocus;
use bevy_ui_text_input::{TextInputNode, TextInputMode};
use crate::components::*;

pub fn setup_chat_ui(mut commands: Commands) {
    // Spawn Camera2d
    commands.spawn(Camera2d);

    // Spawn root container
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.235, 0.235, 0.275)),
        ))
        .with_children(|parent| {
            // Header
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.22)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Chart Analysis"),
                        TextFont::from_font_size(24.0),
                        TextColor(Color::srgb(0.9, 0.9, 0.92)),
                    ));
                });

            // Message area
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip_y(),
                    padding: UiRect::all(Val::Px(10.0)),
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.18, 0.18, 0.20)),
                ScrollPosition::default(),
                MessageContainer,
            ));

            // Input area
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(120.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.22, 0.22, 0.25)),
                    BorderColor::all(Color::srgb(0.35, 0.35, 0.38)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        TextInputNode {
                            clear_on_submit: true,
                            unfocus_on_submit: false,
                            mode: TextInputMode::SingleLine,
                            ..default()
                        },
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.18, 0.20)),
                        ChatInputField,
                        // AutoFocus ensures the text input receives keyboard focus on startup
                        AutoFocus,
                    ));
                });
        });
}
