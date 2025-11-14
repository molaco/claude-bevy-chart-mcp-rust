# Binance Download Module

## Overview

Simplified Binance historical data download functions adapted from flowsurface for CLI use in the data-loader crate.

**Source:** `/home/molaco/Documents/flowsurface-binance/exchange/src/adapter/binance.rs`
**Destination:** `/home/molaco/Documents/bevy-chat-2/data-loader/src/download/binance.rs`

## Changes Made

### 1. Created Files

- **`/home/molaco/Documents/bevy-chat-2/data-loader/src/download/binance.rs`** (1,025 lines)
  - Main download module with simplified functions

- **`/home/molaco/Documents/bevy-chat-2/data-loader/src/lib.rs`**
  - Library interface exposing db and download modules

- **`/home/molaco/Documents/bevy-chat-2/data-loader/examples/binance_download.rs`**
  - Example demonstrating how to use the download functions

### 2. Modified Files

- **`/home/molaco/Documents/bevy-chat-2/data-loader/Cargo.toml`**
  - Added async runtime dependencies:
    - `tokio = { version = "1.41", features = ["full"] }`
    - `tokio-util = { version = "0.7" }`
    - `reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }`
    - `futures = "0.3"`
    - `bytes = "1.9"`
    - `sha2 = "0.10"`

- **`/home/molaco/Documents/bevy-chat-2/data-loader/src/download/mod.rs`**
  - Added `pub mod binance;` to expose the new module

## Key Features

### Functions

1. **`download_klines_multi_date`**
   - Downloads historical kline (candlestick) data for multiple dates
   - Supports parallel downloads (1-10 concurrent)
   - Includes retry logic and SHA256 checksum verification
   - Returns klines grouped by date with statistics

2. **`download_trades_multi_date`**
   - Downloads historical trade data for multiple dates
   - Supports parallel downloads (1-10 concurrent)
   - Includes retry logic and SHA256 checksum verification
   - Returns trades grouped by date with statistics

### Helper Functions

- `download_single_date_klines` - Download klines for a single date
- `download_single_date_trades` - Download trades for a single date
- `build_kline_url` - Construct Binance Vision URL for klines
- `build_trade_url` - Construct Binance Vision URL for trades
- `download_with_retry` - HTTP download with exponential backoff
- `verify_checksum` - SHA256 checksum verification
- `parse_kline_row` - Parse kline CSV row with validation
- `parse_trade_row` - Parse trade CSV row

### Data Structures

```rust
pub enum MarketKind {
    Spot,
    LinearPerps,
    InversePerps,
}

pub struct KlineDownloadResult {
    pub klines_by_date: Vec<(NaiveDate, Vec<Kline>)>,
    pub stats: DownloadStats,
}

pub struct TradeDownloadResult {
    pub trades_by_date: Vec<(NaiveDate, Vec<Trade>)>,
    pub stats: DownloadStats,
}

pub struct DownloadStats {
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
    pub total_records: u64,
    pub total_bytes: u64,
    pub duration: Duration,
    pub errors: Vec<DownloadError>,
}

pub struct Kline {
    pub time: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub taker_buy_volume: f32,
}

pub struct Trade {
    pub time: u64,
    pub price: f32,
    pub qty: f32,
    pub is_sell: bool,
}
```

## Simplifications from Original

1. **Removed GUI dependencies:**
   - No `iced_futures` callbacks
   - No complex progress UI structures
   - Uses simple `log::info!` and `log::debug!` for logging

2. **Simplified type system:**
   - Basic `Kline` and `Trade` structs without complex `Price` wrapper
   - No `TickerInfo` or ticker management
   - Direct f32 values instead of specialized types

3. **Streamlined error handling:**
   - Uses `anyhow::Result` instead of custom `AdapterError`
   - Simpler error messages and propagation

4. **Kept essential features:**
   - Tokio async runtime
   - Parallel downloads with configurable concurrency
   - Cancellation token support
   - SHA256 checksum verification
   - Retry logic with exponential backoff
   - File caching (doesn't re-download existing files)

5. **Enhanced for CLI use:**
   - Clear logging at appropriate levels
   - Statistics tracking (success rate, speed, etc.)
   - Error collection for reporting
   - Date-based organization

## Usage Example

```rust
use chrono::NaiveDate;
use data_loader::download::binance::{download_klines_multi_date, MarketKind};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let result = download_klines_multi_date(
        "BTCUSDT",                                      // symbol
        MarketKind::Spot,                               // market type
        "1h",                                           // interval
        NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(), // start date
        NaiveDate::from_ymd_opt(2024, 11, 3).unwrap(), // end date
        PathBuf::from("./binance_data"),                // base path
        5,                                              // concurrency
        None,                                           // skip_dates
        None,                                           // cancel_token
    )
    .await?;

    println!("Downloaded {} klines from {} files",
        result.stats.total_records,
        result.stats.successful_files
    );

    Ok(())
}
```

## Build Status

✅ **Compilation successful** with only minor warnings (unused code in other modules)

```bash
cd /home/molaco/Documents/bevy-chat-2/data-loader
cargo check                           # Check library
cargo check --example binance_download # Check example
cargo run --example binance_download   # Run example
```

## File Locations

- **Source (flowsurface):** Lines 1865-2281 in `/home/molaco/Documents/flowsurface-binance/exchange/src/adapter/binance.rs`
- **Destination:** `/home/molaco/Documents/bevy-chat-2/data-loader/src/download/binance.rs`
- **Example:** `/home/molaco/Documents/bevy-chat-2/data-loader/examples/binance_download.rs`

## Next Steps

To use the module in your application:

1. Import the module:
   ```rust
   use data_loader::download::binance;
   ```

2. Call download functions with your parameters

3. Process the returned klines/trades data

4. Optionally integrate with the DuckDB storage layer in `data_loader::db`

## Testing

The module has been tested to compile successfully. To test actual downloads:

```bash
RUST_LOG=info cargo run --example binance_download
```

This will download a few days of BTC/USDT 1h klines and display statistics.
