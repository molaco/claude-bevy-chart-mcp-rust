//! minimal text input example

use bevy::{color::palettes::css::NAVY, input_focus::InputFocus, prelude::*};
use bevy_ui_text_input::{
    TextInputContents, TextInputMode, TextInputNode, TextInputPlugin, TextInputPrompt,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TextInputPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut active_input: ResMut<InputFocus>) {
    // UI camera
    commands.spawn(Camera2d);

    let input_entity = commands
        .spawn((
            TextInputNode {
                mode: TextInputMode::SingleLine,
                ..Default::default()
            },
            TextFont {
                font: assets.load("fonts/FiraMono-Medium.ttf"),
                font_size: 25.,
                ..Default::default()
            },
            TextInputPrompt::default(),
            TextInputContents::default(),
            Node {
                width: Val::Px(250.),
                height: Val::Px(30.),
                ..default()
            },
            BackgroundColor(NAVY.into()),
        ))
        .id();

    active_input.set(input_entity);

    commands
        .spawn(Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.),
            column_gap: Val::Px(20.),
            ..Default::default()
        })
        .add_child(input_entity)
        .with_child((
            Node {
                position_type: PositionType::Absolute,
                margin: UiRect::top(Val::Px(100.)),
                ..Default::default()
            },
            Text::new(""),
        ));
}

fn update(
    contents_query: Query<&TextInputContents, Changed<TextInputContents>>,
    mut query: Query<&mut Text>,
) {
    for contents in contents_query.iter() {
        for mut text in query.iter_mut() {
            text.0 = contents.get().to_string();
        }
    }
}
