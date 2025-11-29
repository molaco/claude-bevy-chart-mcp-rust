//! Configuration module for chart settings.
//!
//! This module provides centralized configuration for all chart parameters,
//! replacing hardcoded magic numbers throughout the codebase.
//!
//! # Module Structure
//!
//! - `constants`: Named constants for all numeric values
//! - `theme`: Resource definitions for runtime configuration
//!
//! # Usage
//!
//! ```ignore
//! use crate::config::{constants::*, ChartTheme, ChartDimensions, InteractionConfig};
//!
//! // Use constants directly
//! let zoom_factor = ZOOM_IN_FACTOR;
//!
//! // Or use resources for runtime configuration
//! fn my_system(theme: Res<ChartTheme>) {
//!     let bull_color = theme.bull_candle;
//! }
//! ```

pub mod constants;
pub mod theme;

// Re-export commonly used items at the module level
pub use constants::*;
pub use theme::{ChartDimensions, ChartTheme, InteractionConfig, ZLayerConfig};
