//! Binance data download functions
//!
//! Simplified version adapted from flowsurface for CLI use.
//! Downloads historical kline (candlestick) and trade data from Binance's public data archive.

use chrono::NaiveDate;
use std::collections::HashSet;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use csv::ReaderBuilder;
use futures::stream::{self, StreamExt};

/// Market type for Binance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MarketKind {
    Spot,
    LinearPerps,
    InversePerps,
}

/// Download result containing klines grouped by date
#[derive(Debug)]
pub struct KlineDownloadResult {
    pub klines_by_date: Vec<(NaiveDate, Vec<Kline>)>,
    pub stats: DownloadStats,
}

/// Download result containing trades grouped by date
#[derive(Debug)]
pub struct TradeDownloadResult {
    pub trades_by_date: Vec<(NaiveDate, Vec<Trade>)>,
    pub stats: DownloadStats,
}

/// Summary statistics after download completes
#[derive(Debug, Clone)]
pub struct DownloadStats {
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
    pub total_records: u64,
    pub total_bytes: u64,
    pub duration: Duration,
    pub errors: Vec<DownloadError>,
}

impl DownloadStats {
    pub fn success_rate(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        (self.successful_files as f32 / self.total_files as f32) * 100.0
    }

    pub fn average_speed_mbps(&self) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs > 0.0 {
            (self.total_bytes as f64 / 1_048_576.0) / secs
        } else {
            0.0
        }
    }
}

/// Error that occurred during download
#[derive(Debug, Clone)]
pub struct DownloadError {
    pub file_name: String,
    pub date: NaiveDate,
    pub error: String,
}

/// Simple kline structure
#[derive(Debug, Clone)]
pub struct Kline {
    pub time: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub taker_buy_volume: f32,
}

/// Simple trade structure
#[derive(Debug, Clone)]
pub struct Trade {
    pub time: u64,
    pub price: f32,
    pub qty: f32,
    pub is_sell: bool,
}

/// Download klines for multiple dates with retry logic and checksum verification
///
/// Supports parallel downloads (up to 10 concurrent) and cancellation.
/// Returns downloaded klines grouped by date.
///
/// # Parameters
/// * `symbol` - Trading symbol (e.g., "BTCUSDT")
/// * `market_type` - Type of market (Spot, LinearPerps, InversePerps)
/// * `interval` - Kline interval (e.g., "1m", "5m", "1h", "1d")
/// * `start_date` - Start date (inclusive)
/// * `end_date` - End date (inclusive)
/// * `base_path` - Base directory to save downloaded files
/// * `concurrency` - Number of parallel downloads (1-10)
/// * `skip_dates` - Optional set of dates to skip downloading (for existing data)
/// * `cancel_token` - Optional cancellation token
pub async fn download_klines_multi_date(
    symbol: &str,
    market_type: MarketKind,
    interval: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    base_path: PathBuf,
    concurrency: usize,
    skip_dates: Option<HashSet<NaiveDate>>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
) -> Result<KlineDownloadResult> {
    // Generate date range
    let mut dates = Vec::new();
    let mut current = start_date;
    while current <= end_date {
        dates.push(current);
        current = current
            .succ_opt()
            .context("Date overflow")?;
    }

    // Filter out dates to skip (for existing data)
    let total_dates_requested = dates.len();
    if let Some(ref skip_set) = skip_dates {
        dates.retain(|date| !skip_set.contains(date));
        log::info!(
            "Skipping {} existing dates, downloading {} new dates",
            total_dates_requested - dates.len(),
            dates.len()
        );
    }

    let total_files = dates.len();
    let start_time = Instant::now();

    // Thread-safe progress tracking
    let completed_files = Arc::new(Mutex::new(0usize));
    let failed_files = Arc::new(Mutex::new(0usize));
    let bytes_downloaded = Arc::new(Mutex::new(0u64));
    let records_inserted = Arc::new(Mutex::new(0u64));
    let errors_mutex = Arc::new(Mutex::new(Vec::new()));
    let klines_mutex = Arc::new(Mutex::new(Vec::new()));

    // Limit concurrency to reasonable range
    let concurrency = concurrency.clamp(1, 10);

    // Check for cancellation before starting
    if let Some(ref token) = cancel_token {
        if token.is_cancelled() {
            anyhow::bail!("Download cancelled");
        }
    }

    log::info!(
        "Starting download of {} files with concurrency {}",
        total_files,
        concurrency
    );

    // Process dates in parallel using futures stream
    let _results: Vec<_> = stream::iter(dates)
        .map(|date| {
            let symbol = symbol.to_string();
            let interval = interval.to_string();
            let base_path = base_path.clone();
            let completed_files = Arc::clone(&completed_files);
            let failed_files = Arc::clone(&failed_files);
            let bytes_downloaded = Arc::clone(&bytes_downloaded);
            let records_inserted = Arc::clone(&records_inserted);
            let errors_mutex = Arc::clone(&errors_mutex);
            let klines_mutex = Arc::clone(&klines_mutex);
            let cancel_token = cancel_token.clone();

            async move {
                // Check cancellation
                if let Some(ref token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(anyhow::anyhow!("Cancelled"));
                    }
                }

                log::info!("Downloading klines for {} on {}", symbol, date);

                // Download klines for this date
                let result = download_single_date_klines(
                    &symbol,
                    market_type,
                    &interval,
                    date,
                    base_path,
                )
                .await;

                match result {
                    Ok((klines, bytes)) => {
                        let record_count = klines.len() as u64;

                        // Update progress (thread-safe)
                        {
                            let mut completed = completed_files.lock().unwrap();
                            *completed += 1;
                            let mut bytes_dl = bytes_downloaded.lock().unwrap();
                            *bytes_dl += bytes;
                            let mut records = records_inserted.lock().unwrap();
                            *records += record_count;

                            log::info!(
                                "Downloaded {} klines for {} on {} ({}/{})",
                                record_count,
                                symbol,
                                date,
                                *completed,
                                total_files
                            );
                        }

                        // Store klines (thread-safe)
                        {
                            let mut klines_vec = klines_mutex.lock().unwrap();
                            klines_vec.push((date, klines));
                        }

                        Ok(())
                    }
                    Err(e) => {
                        log::error!("Failed to download klines for {}: {}", date, e);

                        // Update progress and errors (thread-safe)
                        {
                            let mut failed = failed_files.lock().unwrap();
                            *failed += 1;
                        }

                        {
                            let mut errors = errors_mutex.lock().unwrap();
                            errors.push(DownloadError {
                                file_name: format!(
                                    "{}-{}-{}.zip",
                                    symbol.to_uppercase(),
                                    interval,
                                    date.format("%Y-%m-%d")
                                ),
                                date,
                                error: e.to_string(),
                            });
                        }

                        Err(e)
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let completed = *completed_files.lock().unwrap();
    let failed = *failed_files.lock().unwrap();
    let bytes = *bytes_downloaded.lock().unwrap();
    let records = *records_inserted.lock().unwrap();
    let errors = errors_mutex.lock().unwrap().clone();
    let mut klines_by_date = klines_mutex.lock().unwrap().clone();

    // Sort by date for consistent ordering
    klines_by_date.sort_by_key(|(date, _)| *date);

    let stats = DownloadStats {
        total_files,
        successful_files: completed,
        failed_files: failed,
        total_records: records,
        total_bytes: bytes,
        duration: start_time.elapsed(),
        errors,
    };

    log::info!(
        "Download complete: {}/{} files, {} records, {:.2} MB, {:.2}s",
        completed,
        total_files,
        records,
        bytes as f64 / 1_048_576.0,
        start_time.elapsed().as_secs_f64()
    );

    Ok(KlineDownloadResult {
        klines_by_date,
        stats,
    })
}

/// Download trades for multiple dates with parallel processing
///
/// Supports parallel downloads (up to 10 concurrent) and cancellation.
/// Returns downloaded trades grouped by date.
///
/// # Parameters
/// * `symbol` - Trading symbol (e.g., "BTCUSDT")
/// * `market_type` - Type of market (Spot, LinearPerps, InversePerps)
/// * `start_date` - Start date (inclusive)
/// * `end_date` - End date (inclusive)
/// * `base_path` - Base directory to save downloaded files
/// * `concurrency` - Number of parallel downloads (1-10), recommended 3 for trades
/// * `skip_dates` - Optional set of dates to skip downloading (for existing data)
/// * `cancel_token` - Optional cancellation token
pub async fn download_trades_multi_date(
    symbol: &str,
    market_type: MarketKind,
    start_date: NaiveDate,
    end_date: NaiveDate,
    base_path: PathBuf,
    concurrency: usize,
    skip_dates: Option<HashSet<NaiveDate>>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
) -> Result<TradeDownloadResult> {
    // Generate date range
    let mut dates = Vec::new();
    let mut current = start_date;
    while current <= end_date {
        dates.push(current);
        current = current
            .succ_opt()
            .context("Date overflow")?;
    }

    // Filter out dates to skip (for existing data)
    let total_dates_requested = dates.len();
    if let Some(ref skip_set) = skip_dates {
        dates.retain(|date| !skip_set.contains(date));
        log::info!(
            "Skipping {} existing dates, downloading {} new dates",
            total_dates_requested - dates.len(),
            dates.len()
        );
    }

    let total_files = dates.len();
    let start_time = Instant::now();

    // Thread-safe progress tracking
    let completed_files = Arc::new(Mutex::new(0usize));
    let failed_files = Arc::new(Mutex::new(0usize));
    let bytes_downloaded = Arc::new(Mutex::new(0u64));
    let records_inserted = Arc::new(Mutex::new(0u64));
    let errors_mutex = Arc::new(Mutex::new(Vec::new()));
    let trades_mutex = Arc::new(Mutex::new(Vec::new()));

    // Limit concurrency to reasonable range (trades are larger files, use lower concurrency)
    let concurrency = concurrency.clamp(1, 10);

    // Check for cancellation before starting
    if let Some(ref token) = cancel_token {
        if token.is_cancelled() {
            anyhow::bail!("Download cancelled");
        }
    }

    log::info!(
        "Starting download of {} trade files with concurrency {}",
        total_files,
        concurrency
    );

    // Process dates in parallel using futures stream
    let _results: Vec<_> = stream::iter(dates)
        .map(|date| {
            let symbol = symbol.to_string();
            let base_path = base_path.clone();
            let completed_files = Arc::clone(&completed_files);
            let failed_files = Arc::clone(&failed_files);
            let bytes_downloaded = Arc::clone(&bytes_downloaded);
            let records_inserted = Arc::clone(&records_inserted);
            let errors_mutex = Arc::clone(&errors_mutex);
            let trades_mutex = Arc::clone(&trades_mutex);
            let cancel_token = cancel_token.clone();

            async move {
                // Check cancellation
                if let Some(ref token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(anyhow::anyhow!("Cancelled"));
                    }
                }

                log::info!("Downloading trades for {} on {}", symbol, date);

                // Download trades for this date
                let result =
                    download_single_date_trades(&symbol, market_type, date, base_path).await;

                match result {
                    Ok((trades, bytes)) => {
                        let record_count = trades.len() as u64;

                        // Update progress (thread-safe)
                        {
                            let mut completed = completed_files.lock().unwrap();
                            *completed += 1;
                            let mut bytes_dl = bytes_downloaded.lock().unwrap();
                            *bytes_dl += bytes;
                            let mut records = records_inserted.lock().unwrap();
                            *records += record_count;

                            log::info!(
                                "Downloaded {} trades for {} on {} ({}/{})",
                                record_count,
                                symbol,
                                date,
                                *completed,
                                total_files
                            );
                        }

                        // Store trades (thread-safe)
                        {
                            let mut trades_vec = trades_mutex.lock().unwrap();
                            trades_vec.push((date, trades));
                        }

                        Ok(())
                    }
                    Err(e) => {
                        log::error!("Failed to download trades for {}: {}", date, e);

                        // Update progress and errors (thread-safe)
                        {
                            let mut failed = failed_files.lock().unwrap();
                            *failed += 1;
                        }

                        {
                            let mut errors = errors_mutex.lock().unwrap();
                            errors.push(DownloadError {
                                file_name: format!(
                                    "{}-aggTrades-{}.zip",
                                    symbol.to_uppercase(),
                                    date.format("%Y-%m-%d")
                                ),
                                date,
                                error: e.to_string(),
                            });
                        }

                        Err(e)
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let completed = *completed_files.lock().unwrap();
    let failed = *failed_files.lock().unwrap();
    let bytes = *bytes_downloaded.lock().unwrap();
    let records = *records_inserted.lock().unwrap();
    let errors = errors_mutex.lock().unwrap().clone();
    let mut trades_by_date = trades_mutex.lock().unwrap().clone();

    // Sort by date for consistent ordering
    trades_by_date.sort_by_key(|(date, _)| *date);

    let stats = DownloadStats {
        total_files,
        successful_files: completed,
        failed_files: failed,
        total_records: records,
        total_bytes: bytes,
        duration: start_time.elapsed(),
        errors,
    };

    log::info!(
        "Download complete: {}/{} files, {} records, {:.2} MB, {:.2}s",
        completed,
        total_files,
        records,
        bytes as f64 / 1_048_576.0,
        start_time.elapsed().as_secs_f64()
    );

    Ok(TradeDownloadResult {
        trades_by_date,
        stats,
    })
}

/// Download single date klines with retry and checksum verification
async fn download_single_date_klines(
    symbol: &str,
    market_type: MarketKind,
    interval: &str,
    date: NaiveDate,
    base_path: PathBuf,
) -> Result<(Vec<Kline>, u64)> {
    let market_subpath = match market_type {
        MarketKind::Spot => format!("data/spot/daily/klines/{}/{}", symbol, interval),
        MarketKind::LinearPerps => format!("data/futures/um/daily/klines/{}/{}", symbol, interval),
        MarketKind::InversePerps => format!("data/futures/cm/daily/klines/{}/{}", symbol, interval),
    };

    let zip_file_name = format!(
        "{}-{}-{}.zip",
        symbol.to_uppercase(),
        interval,
        date.format("%Y-%m-%d"),
    );

    let base_path = base_path.join(&market_subpath);
    std::fs::create_dir_all(&base_path)
        .context("Failed to create directories")?;

    let base_zip_path = base_path.join(&zip_file_name);

    // Check cache and download if needed
    let bytes_downloaded = if !base_zip_path.exists() {
        let url = build_kline_url(market_type, symbol, interval, date);
        let checksum_url = format!("{}.CHECKSUM", url);

        log::debug!("Downloading from {}", url);

        // Download with retry
        let body = download_with_retry(&url, 3).await?;
        let bytes = body.len() as u64;

        // Verify checksum
        if !verify_checksum(&body, &checksum_url).await? {
            anyhow::bail!("Checksum verification failed");
        }

        // Save to cache
        std::fs::write(&base_zip_path, &body)
            .context("Failed to write zip file")?;

        bytes
    } else {
        // File exists in cache, get size
        let bytes = std::fs::metadata(&base_zip_path)
            .map(|m| m.len())
            .unwrap_or(0);
        log::debug!("Using cached {}", base_zip_path.display());
        bytes
    };

    // Extract and parse
    let file = std::fs::File::open(&base_zip_path)
        .context("Failed to open file")?;

    let mut archive = zip::ZipArchive::new(file)
        .context("Failed to unzip file")?;

    let mut klines = Vec::new();
    for i in 0..archive.len() {
        let csv_file = archive
            .by_index(i)
            .context("Failed to read csv")?;

        let mut csv_reader = ReaderBuilder::new()
            .has_headers(false)
            .from_reader(BufReader::new(csv_file));

        klines.extend(
            csv_reader
                .records()
                .filter_map(|record| record.ok().and_then(|r| parse_kline_row(&r))),
        );
    }

    log::debug!(
        "Loaded {} klines from {} for {}",
        klines.len(),
        date.format("%Y-%m-%d"),
        symbol
    );

    Ok((klines, bytes_downloaded))
}

/// Download single date trades
async fn download_single_date_trades(
    symbol: &str,
    market_type: MarketKind,
    date: NaiveDate,
    base_path: PathBuf,
) -> Result<(Vec<Trade>, u64)> {
    let market_subpath = match market_type {
        MarketKind::Spot => format!("data/spot/daily/aggTrades/{}", symbol),
        MarketKind::LinearPerps => format!("data/futures/um/daily/aggTrades/{}", symbol),
        MarketKind::InversePerps => format!("data/futures/cm/daily/aggTrades/{}", symbol),
    };

    let zip_file_name = format!(
        "{}-aggTrades-{}.zip",
        symbol.to_uppercase(),
        date.format("%Y-%m-%d"),
    );

    let base_path = base_path.join(&market_subpath);
    std::fs::create_dir_all(&base_path)
        .context("Failed to create directories")?;

    let base_zip_path = base_path.join(&zip_file_name);

    // Check cache and download if needed
    let bytes_downloaded = if !base_zip_path.exists() {
        let url = build_trade_url(market_type, symbol, date);
        let checksum_url = format!("{}.CHECKSUM", url);

        log::debug!("Downloading from {}", url);

        // Download with retry
        let body = download_with_retry(&url, 3).await?;
        let bytes = body.len() as u64;

        // Verify checksum
        if !verify_checksum(&body, &checksum_url).await? {
            anyhow::bail!("Checksum verification failed");
        }

        // Save to cache
        std::fs::write(&base_zip_path, &body)
            .context("Failed to write zip file")?;

        bytes
    } else {
        // File exists in cache, get size
        let bytes = std::fs::metadata(&base_zip_path)
            .map(|m| m.len())
            .unwrap_or(0);
        log::debug!("Using cached {}", base_zip_path.display());
        bytes
    };

    // Extract and parse
    let file = std::fs::File::open(&base_zip_path)
        .context("Failed to open file")?;

    let mut archive = zip::ZipArchive::new(file)
        .context("Failed to unzip file")?;

    let mut trades = Vec::new();
    for i in 0..archive.len() {
        let csv_file = archive
            .by_index(i)
            .context("Failed to read csv")?;

        let mut csv_reader = ReaderBuilder::new()
            .has_headers(false)
            .from_reader(BufReader::new(csv_file));

        trades.extend(
            csv_reader
                .records()
                .filter_map(|record| record.ok().and_then(|r| parse_trade_row(&r))),
        );
    }

    log::debug!(
        "Loaded {} trades from {} for {}",
        trades.len(),
        date.format("%Y-%m-%d"),
        symbol
    );

    Ok((trades, bytes_downloaded))
}

/// Build URL for kline data
fn build_kline_url(market: MarketKind, symbol: &str, interval: &str, date: NaiveDate) -> String {
    let market_path = match market {
        MarketKind::Spot => "spot",
        MarketKind::LinearPerps => "futures/um",
        MarketKind::InversePerps => "futures/cm",
    };

    let file_name = format!(
        "{}-{}-{}.zip",
        symbol.to_uppercase(),
        interval,
        date.format("%Y-%m-%d")
    );

    format!(
        "https://data.binance.vision/data/{}/daily/klines/{}/{}/{}",
        market_path,
        symbol.to_uppercase(),
        interval,
        file_name
    )
}

/// Build URL for trade data
fn build_trade_url(market: MarketKind, symbol: &str, date: NaiveDate) -> String {
    let market_path = match market {
        MarketKind::Spot => "spot",
        MarketKind::LinearPerps => "futures/um",
        MarketKind::InversePerps => "futures/cm",
    };

    let file_name = format!(
        "{}-aggTrades-{}.zip",
        symbol.to_uppercase(),
        date.format("%Y-%m-%d")
    );

    format!(
        "https://data.binance.vision/data/{}/daily/aggTrades/{}/{}",
        market_path,
        symbol.to_uppercase(),
        file_name
    )
}

/// Download with retry logic and exponential backoff
async fn download_with_retry(url: &str, max_retries: u32) -> Result<bytes::Bytes> {
    let mut attempt = 0;
    let mut delay = Duration::from_secs(1);

    loop {
        match reqwest::get(url).await {
            Ok(resp) if resp.status().is_success() => {
                return resp.bytes().await.context("Failed to read response bytes");
            }
            Ok(resp) if resp.status() == 404 => {
                // File doesn't exist on Binance Vision (expected for some dates)
                anyhow::bail!("File not found: {}", url);
            }
            Ok(resp) => {
                let status = resp.status();
                if attempt < max_retries {
                    log::warn!(
                        "HTTP {} for {} (attempt {}/{})",
                        status,
                        url,
                        attempt + 1,
                        max_retries
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2; // Exponential backoff
                    attempt += 1;
                } else {
                    anyhow::bail!("HTTP {} after {} retries", status, max_retries);
                }
            }
            Err(e) if attempt < max_retries => {
                log::warn!(
                    "Download failed (attempt {}/{}): {}",
                    attempt + 1,
                    max_retries,
                    e
                );
                tokio::time::sleep(delay).await;
                delay *= 2;
                attempt += 1;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Verify SHA256 checksum
async fn verify_checksum(file_bytes: &[u8], checksum_url: &str) -> Result<bool> {
    use sha2::{Digest, Sha256};

    // Download .CHECKSUM file
    let checksum_text = match reqwest::get(checksum_url).await {
        Ok(resp) if resp.status().is_success() => resp.text().await?,
        Ok(resp) if resp.status() == 404 => {
            log::warn!("Checksum file not found: {}", checksum_url);
            return Ok(true); // Assume valid if checksum not available
        }
        Ok(resp) => {
            log::warn!(
                "Failed to fetch checksum ({}): {}",
                resp.status(),
                checksum_url
            );
            return Ok(true); // Assume valid if checksum not available
        }
        Err(e) => {
            log::warn!("Checksum download error: {}", e);
            return Ok(true); // Assume valid if checksum not available
        }
    };

    // Parse checksum file: "abc123def456  BTCUSDT-1h-2024-10-16.zip"
    let expected_hash = checksum_text
        .split_whitespace()
        .next()
        .context("Invalid checksum format")?;

    // Calculate SHA256 of downloaded file
    let mut hasher = Sha256::new();
    hasher.update(file_bytes);
    let computed_hash = format!("{:x}", hasher.finalize());

    Ok(computed_hash == expected_hash)
}

/// Parse a kline CSV row
fn parse_kline_row(record: &csv::StringRecord) -> Option<Kline> {
    if record.len() != 12 {
        log::warn!(
            "Invalid kline row: expected 12 columns, got {}",
            record.len()
        );
        return None;
    }

    let time = record[0].parse::<u64>().ok()?;
    let open = record[1].parse::<f32>().ok()?;
    let high = record[2].parse::<f32>().ok()?;
    let low = record[3].parse::<f32>().ok()?;
    let close = record[4].parse::<f32>().ok()?;
    let volume = record[5].parse::<f32>().ok()?;
    let taker_buy_volume = record[9].parse::<f32>().ok()?;

    // Validate OHLC relationship
    if high < low || high < open || high < close || low > open || low > close {
        log::warn!(
            "Invalid OHLC at {}: O={} H={} L={} C={}",
            time,
            open,
            high,
            low,
            close
        );
        return None;
    }

    Some(Kline {
        time,
        open,
        high,
        low,
        close,
        volume,
        taker_buy_volume,
    })
}

/// Parse a trade CSV row
fn parse_trade_row(record: &csv::StringRecord) -> Option<Trade> {
    if record.len() < 7 {
        log::warn!(
            "Invalid trade row: expected at least 7 columns, got {}",
            record.len()
        );
        return None;
    }

    // Binance aggTrades CSV format (8 columns):
    // agg_trade_id, price, quantity, first_trade_id, last_trade_id, timestamp, is_buyer_maker, is_best_match
    let time = record.get(5)?.parse::<u64>().ok()?;
    let price = record.get(1)?.parse::<f32>().ok()?;
    let qty = record.get(2)?.parse::<f32>().ok()?;

    // Binance uses "True"/"False" (capital T/F), not "true"/"false"
    let is_buyer_maker_str = record.get(6)?;
    let is_buyer_maker = match is_buyer_maker_str {
        "True" | "true" => true,
        "False" | "false" => false,
        _ => {
            log::warn!("Invalid is_buyer_maker value: {}", is_buyer_maker_str);
            return None;
        }
    };

    Some(Trade {
        time,
        price,
        qty,
        is_sell: is_buyer_maker, // buyer maker = sell (taker is seller)
    })
}
