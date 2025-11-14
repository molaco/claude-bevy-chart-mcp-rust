//! Example: Download Binance klines data
//!
//! Run with: cargo run --example binance_download

use chrono::NaiveDate;
use data_loader::download::binance::{download_klines_multi_date, MarketKind};
use std::path::PathBuf;

// Re-export for examples (Cargo normalizes crate names with hyphens to underscores)
use data_loader as data_loader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::init();

    // Configuration
    let symbol = "BTCUSDT";
    let market_type = MarketKind::Spot;
    let interval = "1h";
    let start_date = NaiveDate::from_ymd_opt(2024, 11, 1).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2024, 11, 3).unwrap();
    let base_path = PathBuf::from("./binance_data");
    let concurrency = 5;

    println!("Downloading {} {} klines from {} to {}",
        symbol, interval, start_date, end_date);

    // Download klines
    let result = download_klines_multi_date(
        symbol,
        market_type,
        interval,
        start_date,
        end_date,
        base_path,
        concurrency,
        None, // skip_dates
        None, // cancel_token
    )
    .await?;

    // Print statistics
    println!("\n=== Download Complete ===");
    println!("Total files: {}", result.stats.total_files);
    println!("Successful: {}", result.stats.successful_files);
    println!("Failed: {}", result.stats.failed_files);
    println!("Total records: {}", result.stats.total_records);
    println!("Total bytes: {:.2} MB", result.stats.total_bytes as f64 / 1_048_576.0);
    println!("Duration: {:.2}s", result.stats.duration.as_secs_f64());
    println!("Average speed: {:.2} MB/s", result.stats.average_speed_mbps());
    println!("Success rate: {:.1}%", result.stats.success_rate());

    // Print sample klines
    println!("\n=== Sample Klines ===");
    for (date, klines) in result.klines_by_date.iter().take(1) {
        println!("Date: {}", date);
        for kline in klines.iter().take(5) {
            println!(
                "  Time: {}, O: {:.2}, H: {:.2}, L: {:.2}, C: {:.2}, V: {:.2}",
                kline.time, kline.open, kline.high, kline.low, kline.close, kline.volume
            );
        }
        println!("  ... ({} total klines)", klines.len());
    }

    // Print errors if any
    if !result.stats.errors.is_empty() {
        println!("\n=== Errors ===");
        for error in &result.stats.errors {
            println!("  {}: {}", error.file_name, error.error);
        }
    }

    Ok(())
}
