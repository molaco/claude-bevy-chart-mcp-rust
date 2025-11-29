//! Domain module - Pure Rust business types with zero Bevy dependencies.
//!
//! This module contains core data structures that represent the business domain
//! of candlestick charting. These types have no dependencies on Bevy or any
//! rendering framework, making them portable and easy to test.
//!
//! # Contents
//!
//! - [`Candle`]: OHLCV candlestick data with validation
//! - [`Timeframe`]: Type-safe timeframe representation

mod candle;
mod timeframe;

pub use candle::Candle;
pub use timeframe::Timeframe;
