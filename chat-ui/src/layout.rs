use crate::components::*;
use bevy::input_focus::AutoFocus;
use bevy::prelude::*;
use bevy_ui_text_input::{TextInputMode, TextInputNode};

/// Marker component to identify the chat UI container
#[derive(Component)]
pub struct ChatUiRoot;

pub fn setup_chat_ui(mut commands: Commands) {
    // Chat UI positioned within its container
    // 50% of container width, 70% of height, left-aligned horizontally, centered vertically
    commands
        .spawn((
            Node {
                width: Val::Percent(90.0),  // 50% of container width
                height: Val::Percent(70.0), // 70% of container height
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                // margin: UiRect::left(Val::Px(20.0)),
                // align_self: AlignSelf::Start, // aligns to left within container
                // align_self removed - will use parent's centering for vertical, left by default for horizontal
                ..default()
            },
            // BackgroundColor(Color::srgb(0.235, 0.235, 0.275)),
            // BackgroundColor(Color::srgb(0.5, 0.5, 0.5)),
            BackgroundColor(Color::srgb_u8(43, 44, 47)),
            BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 0.6)),
            ChatUiRoot,
        ))
        .with_children(|parent| {
            // Message area
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip_y(),
                    padding: UiRect::all(Val::Px(10.0)),
                    margin: UiRect::bottom(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                // BackgroundColor(Color::NONE),
                BackgroundColor(Color::srgb_u8(43, 44, 47)),
                // BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
                BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 0.6)),
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
                    // BackgroundColor(Color::srgb(0.22, 0.22, 0.25)),
                    // BackgroundColor(Color::NONE),
                    BackgroundColor(Color::srgb_u8(43, 44, 47)),
                    // BorderColor::all(Color::NONE),
                    BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 0.6)),
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
                        BackgroundColor(Color::NONE),
                        ChatInputField,
                        // AutoFocus ensures the text input receives keyboard focus on startup
                        AutoFocus,
                    ));
                });
        });
}

// Note: reparent_chat_to_container removed - now handled in charts/src/ui_layout.rs
