use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

/// Take a screenshot of the primary window and save it to the specified path.
/// If no path is provided, generates an auto-incrementing filename.
/// Returns the path where the screenshot was saved.
pub fn take_screenshot(commands: &mut Commands, path: Option<String>, next_index: &mut u32) -> String {
    let filename = path.unwrap_or_else(|| {
        let name = format!("screenshot-{:04}.png", *next_index);
        *next_index += 1;
        name
    });

    println!("Taking screenshot: {}", filename);

    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(filename.clone()));

    filename
}
