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

/// Parses Binance ZIP archives and bulk-loads trades into database
pub struct ArchiveImporter {
    batch_size: usize,
    dry_run: bool,
}

impl ArchiveImporter {
    pub fn new(batch_size: usize, dry_run: bool) -> Self {
        Self {
            batch_size,
            dry_run,
        }
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

            let count = match self.stream_csv_insert(conn, file, ticker_id) {
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
    ) -> Result<usize> {
        let tx = conn.transaction()
            .context("Failed to start transaction")?;

        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);

        let mut batch = Vec::new();
        let mut count = 0;
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
            let quantity: f32 = record[2].parse().unwrap_or(0.0);
            let timestamp: i64 = record[5].parse().unwrap_or(0);
            let is_buyer_maker: bool = record[6].parse().unwrap_or(false);
            let is_sell = is_buyer_maker; // Buyer maker means it's a sell

            // Generate deterministic ID for idempotent inserts
            let db_trade_id = generate_trade_id(ticker_id, timestamp as u64, trade_counter);
            trade_counter += 1;

            batch.push((db_trade_id, ticker_id, timestamp, price, quantity, is_sell));

            if batch.len() >= self.batch_size {
                self.insert_trade_batch(&tx, &batch)?;
                count += batch.len();
                batch.clear();
            }
        }

        // Insert remaining batch
        if !batch.is_empty() {
            self.insert_trade_batch(&tx, &batch)?;
            count += batch.len();
        }

        tx.commit()
            .context("Failed to commit trades transaction")?;

        Ok(count)
    }

    fn insert_trade_batch(
        &self,
        conn: &duckdb::Transaction,
        batch: &[(i64, i64, i64, f64, f32, bool)],
    ) -> Result<()> {
        let mut stmt = conn
            .prepare(
                "INSERT OR REPLACE INTO trades
                 (trade_id, ticker_id, timestamp, price, quantity, is_sell)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .context("Failed to prepare statement")?;

        for row in batch {
            stmt.execute(duckdb::params![row.0, row.1, row.2, row.3, row.4, row.5])
                .context("Failed to insert trade")?;
        }

        Ok(())
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
        let importer = ArchiveImporter::new(1000, false);
        assert_eq!(importer.batch_size, 1000);
        assert!(!importer.dry_run);
    }

    #[test]
    fn test_parse_archive_path() {
        let importer = ArchiveImporter::new(1000, false);

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
        let importer = ArchiveImporter::new(1000, false);

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
