//! Data module - Database access and data storage.
//!
//! This module provides database connectivity and data storage for
//! candlestick chart data, with support for lazy loading.
//!
//! # Contents
//!
//! - [`ChartDatabase`]: DuckDB database wrapper for loading candles
//! - [`CandleData`]: Bevy resource for storing loaded candle data

mod database;
mod store;

pub use database::ChartDatabase;
pub use store::CandleData;
