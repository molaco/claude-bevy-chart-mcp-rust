//! Binance data download module
//!
//! Downloads historical kline (candlestick) and trade data from Binance's public data archive.

pub mod binance;
mod types;
pub mod progress;

#[allow(unused_imports)]
pub use types::{DownloadProgress, DownloadResult};
