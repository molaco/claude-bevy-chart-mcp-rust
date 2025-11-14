//! ArchiveImporter for Binance ZIP archives
//!
//! Parses Binance ZIP archives and bulk-loads trades into database

use super::helpers::{generate_trade_id, get_or_create_ticker_id};
use anyhow::{Context, Result};
use duckdb::Connection;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Simple migration statistics
#[derive(Debug, Clone, Default)]
pub struct ImportStats {
    pub files_processed: usize,
    pub trades_migrated: usize,
    pub errors: Vec<String>,
}

impl ImportStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn merge(&mut self, other: &ImportStats) {
        self.files_processed += other.files_processed;
        self.trades_migrated += other.trades_migrated;
        self.errors.extend(other.errors.clone());
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }
}

/// Simple progress tracker
struct ProgressTracker {
    total: usize,
    current: usize,
    name: String,
}

impl ProgressTracker {
    fn new(total: usize, name: &str) -> Self {
        log::info!("{}: Processing {} items", name, total);
        Self {
            total,
            current: 0,
            name: name.to_string(),
        }
    }

    fn update(&mut self, count: usize) {
        self.current += count;
        if self.current % 10 == 0 || self.current == self.total {
            log::info!(
                "{}: {}/{} ({:.1}%)",
                self.name,
                self.current,
                self.total,
                (self.current as f64 / self.total as f64) * 100.0
            );
        }
    }

    fn finish(&self) {
        log::info!("{}: Complete ({}/{})", self.name, self.current, self.total);
    }
}

/// Data type to import
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Trades,
    Klines,
}

/// Parses Binance ZIP archives and bulk-loads trades or klines into database
pub struct ArchiveImporter {
    batch_size: usize,
    dry_run: bool,
    data_type: DataType,
}

impl ArchiveImporter {
    pub fn new(batch_size: usize, dry_run: bool, data_type: DataType) -> Self {
        Self {
            batch_size,
            dry_run,
            data_type,
        }
    }

    /// Create a new importer for trades
    pub fn new_for_trades(batch_size: usize, dry_run: bool) -> Self {
        Self::new(batch_size, dry_run, DataType::Trades)
    }

    /// Create a new importer for klines
    pub fn new_for_klines(batch_size: usize, dry_run: bool) -> Self {
        Self::new(batch_size, dry_run, DataType::Klines)
    }

    /// Walk directory tree finding all ZIP files and import each one
    ///
    /// Returns aggregated statistics across all processed archives
    pub fn import_zip_archives(
        &self,
        conn: &mut Connection,
        market_data_path: &Path,
    ) -> Result<ImportStats> {
        log::info!(
            "Scanning for ZIP archives in: {}",
            market_data_path.display()
        );

        let mut all_stats = ImportStats::new();

        if !market_data_path.exists() {
            let err_msg = format!("Market data path does not exist: {}", market_data_path.display());
            log::warn!("{}", err_msg);
            all_stats.add_error(err_msg);
            return Ok(all_stats);
        }

        // Collect all ZIP files
        let zip_files = self.find_zip_files(market_data_path)?;
        log::info!("Found {} ZIP archives to process", zip_files.len());

        if zip_files.is_empty() {
            return Ok(all_stats);
        }

        let mut tracker = ProgressTracker::new(zip_files.len(), "Archive Import");

        for (i, zip_path) in zip_files.iter().enumerate() {
            log::debug!("Processing archive {}/{}: {}", i + 1, zip_files.len(), zip_path.display());

            match self.import_single_archive(conn, zip_path) {
                Ok(stats) => {
                    all_stats.merge(&stats);
                    all_stats.files_processed += 1;
                }
                Err(e) => {
                    let err_msg = format!("{}: {}", zip_path.display(), e);
                    log::error!("Failed to import archive: {}", err_msg);
                    all_stats.add_error(err_msg);
                }
            }

            tracker.update(1);
        }

        tracker.finish();
        log::info!(
            "Archive import complete: {} files processed, {} trades imported",
            all_stats.files_processed,
            all_stats.trades_migrated
        );

        Ok(all_stats)
    }

    /// Process a single ZIP file containing Binance aggTrades CSV
    ///
    /// Streams CSV parsing to avoid memory issues on large files (500MB+)
    pub fn import_single_archive(&self, conn: &mut Connection, zip_path: &Path) -> Result<ImportStats> {
        let mut stats = ImportStats::new();

        if self.dry_run {
            log::info!("Dry run mode - skipping archive: {}", zip_path.display());
            return Ok(stats);
        }

        // Parse metadata from path
        let (symbol, exchange_name) = match self.parse_archive_path(zip_path) {
            Ok(result) => result,
            Err(e) => {
                let err_msg = format!("Failed to parse archive path: {}", e);
                stats.add_error(err_msg);
                return Ok(stats);
            }
        };

        // Get or create ticker_id with default values
        let ticker_id = match get_or_create_ticker_id(conn, &symbol, &exchange_name, 0.01, 0.001, None) {
            Ok(id) => id,
            Err(e) => {
                let err_msg = format!("Failed to get ticker_id: {}", e);
                stats.add_error(err_msg);
                return Ok(stats);
            }
        };

        // Open ZIP archive
        let file = fs::File::open(zip_path)
            .context(format!("Failed to open ZIP archive: {}", zip_path.display()))?;

        let mut archive = zip::ZipArchive::new(file)
            .context("Failed to read ZIP archive")?;

        // Process each file in archive (typically just one CSV)
        for i in 0..archive.len() {
            let file = archive.by_index(i)
                .context("Failed to access archive entry")?;

            // Extract timeframe from path for klines
            let timeframe = if self.data_type == DataType::Klines {
                self.extract_timeframe_from_path(zip_path).unwrap_or_else(|_| "1h".to_string())
            } else {
                String::new()
            };

            let count = match self.stream_csv_insert(conn, file, ticker_id, &timeframe) {
                Ok(c) => c,
                Err(e) => {
                    let err_msg = format!("Failed to process CSV: {}", e);
                    stats.add_error(err_msg);
                    return Ok(stats);
                }
            };

            stats.trades_migrated += count;
        }

        Ok(stats)
    }

    /// Extract timeframe from klines archive path
    ///
    /// Path format for klines: .../klines/BTCUSDT/1h/BTCUSDT-1h-2024-01-01.zip
    /// Returns timeframe (e.g., "1h", "1d")
    fn extract_timeframe_from_path(&self, zip_path: &Path) -> Result<String> {
        let file_name = zip_path
            .file_name()
            .and_then(|s| s.to_str())
            .context("Invalid file name")?;

        // Extract timeframe from filename (e.g., BTCUSDT-1h-2024-01-01.zip)
        let parts: Vec<&str> = file_name.split('-').collect();
        if parts.len() >= 2 {
            Ok(parts[1].to_string())
        } else {
            anyhow::bail!("Cannot extract timeframe from filename: {}", file_name);
        }
    }

    /// Extract symbol and exchange from Binance archive filesystem path
    ///
    /// Path format: market_data/binance/data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.zip
    /// Returns (symbol, exchange_name)
    pub fn parse_archive_path(&self, zip_path: &Path) -> Result<(String, String)> {
        let file_name = zip_path
            .file_name()
            .and_then(|s| s.to_str())
            .context("Invalid file name")?;

        // Extract symbol from filename (e.g., BTCUSDT-aggTrades-2024-01-15.zip)
        let parts: Vec<&str> = file_name.split('-').collect();
        if parts.len() < 4 {
            anyhow::bail!("Invalid Binance archive filename format: {}", file_name);
        }

        let symbol = parts[0].to_string();

        // Determine exchange from path
        let path_str = zip_path.to_string_lossy();
        let exchange_name = if path_str.contains("/futures/") || path_str.contains("\\futures\\") {
            "BinanceLinear"
        } else if path_str.contains("/spot/") || path_str.contains("\\spot\\") {
            "BinanceSpot"
        } else {
            // Default to futures if can't determine
            "BinanceLinear"
        };

        Ok((symbol, exchange_name.to_string()))
    }

    /// Stream CSV records and insert in batches
    ///
    /// Avoids loading entire file into memory
    fn stream_csv_insert<R: Read>(
        &self,
        conn: &mut Connection,
        reader: R,
        ticker_id: i64,
        timeframe: &str,
    ) -> Result<usize> {
        match self.data_type {
            DataType::Trades => self.stream_trades_csv(conn, reader, ticker_id),
            DataType::Klines => self.stream_klines_csv(conn, reader, ticker_id, timeframe),
        }
    }

    /// Stream trades CSV records and insert in batches using Appender API
    fn stream_trades_csv<R: Read>(
        &self,
        conn: &mut Connection,
        reader: R,
        ticker_id: i64,
    ) -> Result<usize> {
        // Create temporary table for bulk loading
        let temp_table = format!("temp_trades_{}", std::process::id());
        conn.execute_batch(&format!(
            "CREATE TEMPORARY TABLE {} (
                trade_id BIGINT,
                ticker_id INTEGER,
                timestamp BIGINT,
                price DECIMAL(18,8),
                quantity DECIMAL(18,8),
                is_buyer_maker BOOLEAN
            )",
            temp_table
        ))
        .context("Failed to create temp table")?;

        // Use Appender for bulk loading into temp table
        let mut appender = conn.appender(&temp_table)
            .context("Failed to create appender")?;

        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);

        let mut trade_counter: u64 = 0;

        for result in csv_reader.records() {
            let record = result
                .context("Failed to parse CSV record")?;

            if record.len() < 7 {
                continue; // Skip invalid records
            }

            // Binance aggTrades CSV format:
            // agg_trade_id, price, quantity, first_trade_id, last_trade_id, timestamp, is_buyer_maker
            let _trade_id: i64 = record[0].parse().unwrap_or(0);
            let price: f64 = record[1].parse().unwrap_or(0.0);
            let quantity: f64 = record[2].parse().unwrap_or(0.0);
            let timestamp: i64 = record[5].parse().unwrap_or(0);
            let is_buyer_maker: bool = record[6].parse().unwrap_or(false);

            // Generate deterministic ID for idempotent inserts
            let db_trade_id = generate_trade_id(ticker_id, timestamp as u64, trade_counter);
            trade_counter += 1;

            appender.append_row(duckdb::params![
                db_trade_id,
                ticker_id,
                timestamp,
                price,
                quantity,
                is_buyer_maker
            ])
            .context("Failed to append trade row")?;
        }

        // Flush appender to commit data to temp table
        appender.flush()
            .context("Failed to flush appender")?;

        // Drop appender to release lock
        drop(appender);

        // Upsert from temp table to main table
        let rows_affected = conn.execute(
            &format!(
                "INSERT INTO trades (trade_id, ticker_id, timestamp, price, quantity, is_buyer_maker)
                 SELECT trade_id, ticker_id, timestamp, price, quantity, is_buyer_maker
                 FROM {}
                 ON CONFLICT (trade_id) DO UPDATE SET
                     price = EXCLUDED.price,
                     quantity = EXCLUDED.quantity,
                     is_buyer_maker = EXCLUDED.is_buyer_maker",
                temp_table
            ),
            [],
        )
        .context("Failed to upsert from temp table")?;

        // Drop temp table
        conn.execute_batch(&format!("DROP TABLE {}", temp_table))
            .context("Failed to drop temp table")?;

        Ok(rows_affected)
    }

    /// Stream klines CSV records and insert in batches using Appender API
    fn stream_klines_csv<R: Read>(
        &self,
        conn: &mut Connection,
        reader: R,
        ticker_id: i64,
        timeframe: &str,
    ) -> Result<usize> {
        // Create temporary table for bulk loading
        let temp_table = format!("temp_klines_{}", std::process::id());
        conn.execute_batch(&format!(
            "CREATE TEMPORARY TABLE {} (
                kline_id BIGINT,
                ticker_id INTEGER,
                timeframe VARCHAR,
                candle_time BIGINT,
                open_price DECIMAL(18,8),
                high_price DECIMAL(18,8),
                low_price DECIMAL(18,8),
                close_price DECIMAL(18,8),
                volume DECIMAL(18,8),
                num_trades INTEGER
            )",
            temp_table
        ))
        .context("Failed to create temp table")?;

        // Use Appender for bulk loading into temp table
        let mut appender = conn.appender(&temp_table)
            .context("Failed to create appender")?;

        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);

        let mut kline_counter: u64 = 0;

        for result in csv_reader.records() {
            let record = result
                .context("Failed to parse CSV record")?;

            if record.len() < 12 {
                continue; // Skip invalid records
            }

            // Binance klines CSV format:
            // open_time, open, high, low, close, volume, close_time, quote_volume, trades, taker_buy_base, taker_buy_quote, ignore
            let open_time: i64 = record[0].parse().unwrap_or(0);
            let open: f64 = record[1].parse().unwrap_or(0.0);
            let high: f64 = record[2].parse().unwrap_or(0.0);
            let low: f64 = record[3].parse().unwrap_or(0.0);
            let close: f64 = record[4].parse().unwrap_or(0.0);
            let volume: f64 = record[5].parse().unwrap_or(0.0);
            let _close_time: i64 = record[6].parse().unwrap_or(0);
            let num_trades: i32 = record[8].parse().unwrap_or(0);

            // Generate deterministic ID for idempotent inserts
            let kline_id = super::helpers::generate_kline_id(ticker_id, timeframe, open_time as u64, kline_counter);
            kline_counter += 1;

            appender.append_row(duckdb::params![
                kline_id,
                ticker_id,
                timeframe,
                open_time,
                open,
                high,
                low,
                close,
                volume,
                num_trades
            ])
            .context("Failed to append kline row")?;
        }

        // Flush appender to commit data to temp table
        appender.flush()
            .context("Failed to flush appender")?;

        // Drop appender to release lock
        drop(appender);

        // Upsert from temp table to main table
        let rows_affected = conn.execute(
            &format!(
                "INSERT INTO klines (kline_id, ticker_id, timeframe, candle_time, open_price, high_price, low_price, close_price, volume, num_trades)
                 SELECT kline_id, ticker_id, timeframe, candle_time, open_price, high_price, low_price, close_price, volume, num_trades
                 FROM {}
                 ON CONFLICT (ticker_id, timeframe, candle_time) DO UPDATE SET
                     open_price = EXCLUDED.open_price,
                     high_price = EXCLUDED.high_price,
                     low_price = EXCLUDED.low_price,
                     close_price = EXCLUDED.close_price,
                     volume = EXCLUDED.volume,
                     num_trades = EXCLUDED.num_trades",
                temp_table
            ),
            [],
        )
        .context("Failed to upsert from temp table")?;

        // Drop temp table
        conn.execute_batch(&format!("DROP TABLE {}", temp_table))
            .context("Failed to drop temp table")?;

        Ok(rows_affected)
    }


    /// Find all ZIP files recursively in directory
    fn find_zip_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut zip_files = Vec::new();

        if !root.is_dir() {
            return Ok(zip_files);
        }

        let entries = fs::read_dir(root)
            .context(format!("Failed to read directory: {}", root.display()))?;

        for entry in entries {
            let entry = entry
                .context("Failed to read directory entry")?;

            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                let sub_files = self.find_zip_files(&path)?;
                zip_files.extend(sub_files);
            } else if path.extension().and_then(|s| s.to_str()) == Some("zip") {
                zip_files.push(path);
            }
        }

        Ok(zip_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_importer_creation() {
        let importer = ArchiveImporter::new_for_trades(1000, false);
        assert_eq!(importer.batch_size, 1000);
        assert!(!importer.dry_run);
        assert_eq!(importer.data_type, DataType::Trades);

        let importer = ArchiveImporter::new_for_klines(1000, false);
        assert_eq!(importer.batch_size, 1000);
        assert!(!importer.dry_run);
        assert_eq!(importer.data_type, DataType::Klines);
    }

    #[test]
    fn test_parse_archive_path() {
        let importer = ArchiveImporter::new_for_trades(1000, false);

        let path = PathBuf::from(
            "market_data/binance/data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.zip",
        );

        let result = importer.parse_archive_path(&path);
        assert!(result.is_ok());

        let (symbol, exchange) = result.unwrap();
        assert_eq!(symbol, "BTCUSDT");
        assert_eq!(exchange, "BinanceLinear");
    }

    #[test]
    fn test_parse_archive_path_spot() {
        let importer = ArchiveImporter::new_for_trades(1000, false);

        let path = PathBuf::from(
            "market_data/binance/data/spot/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.zip",
        );

        let result = importer.parse_archive_path(&path);
        assert!(result.is_ok());

        let (symbol, exchange) = result.unwrap();
        assert_eq!(symbol, "BTCUSDT");
        assert_eq!(exchange, "BinanceSpot");
    }
}
