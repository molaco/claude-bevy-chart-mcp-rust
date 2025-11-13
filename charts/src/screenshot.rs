use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use image::ImageReader;

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

/// Crop a screenshot from source path to destination path, keeping only the chart area (left 70%).
/// Returns an error if the file cannot be read or written.
pub fn crop_screenshot_to_chart(source_path: &str, dest_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Load the saved screenshot
    let img = ImageReader::open(source_path)?.decode()?;

    // Get dimensions
    let width = img.width();
    let height = img.height();

    // Crop to left 70% (chart area only)
    let crop_width = (width as f32 * 0.7) as u32;
    let cropped = img.crop_imm(0, 0, crop_width, height);

    // Save to destination path
    cropped.save(dest_path)?;

    println!("Cropped screenshot to chart area: {}x{} -> {}x{} (saved to {})", width, height, crop_width, height, dest_path);
    Ok(())
}
