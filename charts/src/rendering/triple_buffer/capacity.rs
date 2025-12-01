//! Buffer capacity calculation utilities.
//!
//! This module provides functions for calculating buffer sizes with
//! growth factors to reduce reallocation frequency.

use super::constants::{BUFFER_GROWTH_FACTOR, MIN_BUFFER_CAPACITY};

/// Calculates the new buffer capacity with growth factor
///
/// Given a required size, returns a capacity that:
/// 1. Is at least `required_size`
/// 2. Is at least `MIN_BUFFER_CAPACITY`
/// 3. Applies `BUFFER_GROWTH_FACTOR` to provide headroom
///
/// # Arguments
/// * `required_size` - The minimum size needed in bytes
///
/// # Returns
/// The recommended capacity including growth headroom
pub fn calculate_grown_capacity(required_size: usize) -> usize {
    let grown = (required_size as f32 * BUFFER_GROWTH_FACTOR).ceil() as usize;
    grown.max(MIN_BUFFER_CAPACITY)
}

/// Calculates capacity for initial pre-allocation
///
/// For initial allocation, we want extra headroom to handle typical
/// data size variations without immediate reallocation.
///
/// # Arguments
/// * `required_size` - The size of initial data in bytes
///
/// # Returns
/// The recommended initial capacity
pub fn calculate_initial_capacity(required_size: usize) -> usize {
    // For initial allocation, use 2x growth factor for more headroom
    let grown = (required_size as f32 * BUFFER_GROWTH_FACTOR * BUFFER_GROWTH_FACTOR).ceil() as usize;
    grown.max(MIN_BUFFER_CAPACITY)
}

/// Formats a byte count as a human-readable string
pub fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
