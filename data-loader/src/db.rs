//! Database module for DuckDB operations

use anyhow::{Context, Result};
use duckdb::Connection;
use std::path::Path;

/// Database statistics
#[derive(Debug, Clone)]
pub struct DbStats {
    pub total_trades: i64,
    pub total_klines: i64,
    pub total_tickers: i64,
    pub database_size_bytes: i64,
    pub schema_version: i32,
}

/// Per-ticker statistics
#[derive(Debug, Clone)]
pub struct TickerStats {
    pub symbol: String,
    pub exchange: String,
    pub trade_count: i64,
    pub trade_start_date: Option<String>,
    pub trade_end_date: Option<String>,
    pub kline_intervals: Vec<KlineIntervalStats>,
}

/// Kline statistics by interval
#[derive(Debug, Clone)]
pub struct KlineIntervalStats {
    pub interval: String,
    pub count: i64,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// DatabaseManager for DuckDB operations
pub struct DatabaseManager {
    conn: Connection,
    db_path: std::path::PathBuf,
}

impl DatabaseManager {
    /// Create a new DatabaseManager
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create database directory: {}", parent.display()))?;
        }

        // Open or create database
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

        let manager = Self { conn, db_path };

        // Initialize schema if needed
        manager.initialize_schema()?;

        Ok(manager)
    }

    /// Initialize database schema
    fn initialize_schema(&self) -> Result<()> {
        // Check if schema is already initialized
        let has_schema: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM information_schema.tables WHERE table_name = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if has_schema {
            log::debug!("Schema already initialized");
            return Ok(());
        }

        log::info!("Initializing database schema...");

        // Create schema version table
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS exchanges (
                exchange_id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS tickers (
                ticker_id INTEGER PRIMARY KEY,
                exchange_id INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                tick_size DOUBLE NOT NULL,
                min_quantity DOUBLE NOT NULL,
                contract_size DOUBLE,
                FOREIGN KEY (exchange_id) REFERENCES exchanges(exchange_id),
                UNIQUE(exchange_id, symbol)
            );

            CREATE TABLE IF NOT EXISTS trades (
                trade_id BIGINT PRIMARY KEY,
                ticker_id INTEGER NOT NULL,
                timestamp BIGINT NOT NULL,
                price DOUBLE NOT NULL,
                quantity FLOAT NOT NULL,
                is_sell BOOLEAN NOT NULL,
                FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id)
            );

            CREATE TABLE IF NOT EXISTS klines (
                kline_id BIGINT PRIMARY KEY,
                ticker_id INTEGER NOT NULL,
                timeframe TEXT NOT NULL,
                open_time BIGINT NOT NULL,
                close_time BIGINT NOT NULL,
                open DOUBLE NOT NULL,
                high DOUBLE NOT NULL,
                low DOUBLE NOT NULL,
                close DOUBLE NOT NULL,
                volume FLOAT NOT NULL,
                FOREIGN KEY (ticker_id) REFERENCES tickers(ticker_id),
                UNIQUE(ticker_id, timeframe, open_time)
            );

            -- Insert initial schema version
            INSERT INTO schema_version (version) VALUES (1);
            "#,
        )?;

        log::info!("Schema initialized successfully");
        Ok(())
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DbStats> {
        let total_trades: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))
            .unwrap_or(0);

        let total_klines: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM klines", [], |row| row.get(0))
            .unwrap_or(0);

        let total_tickers: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tickers", [], |row| row.get(0))
            .unwrap_or(0);

        let database_size_bytes: i64 = std::fs::metadata(&self.db_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let schema_version: i32 = self
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        Ok(DbStats {
            total_trades,
            total_klines,
            total_tickers,
            database_size_bytes,
            schema_version,
        })
    }

    /// Vacuum and optimize database
    pub fn vacuum(&self) -> Result<()> {
        log::info!("Running VACUUM...");
        self.conn.execute_batch("VACUUM; ANALYZE;")?;
        log::info!("VACUUM completed");
        Ok(())
    }

    /// Get statistics for all tickers
    pub fn get_all_ticker_stats(&self) -> Result<Vec<TickerStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.symbol, e.name as exchange, t.ticker_id
             FROM tickers t
             JOIN exchanges e ON t.exchange_id = e.exchange_id
             ORDER BY t.symbol"
        )?;

        let ticker_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // symbol
                row.get::<_, String>(1)?, // exchange
                row.get::<_, i64>(2)?,    // ticker_id
            ))
        })?;

        let mut stats = Vec::new();

        for ticker_row in ticker_rows {
            let (symbol, exchange, ticker_id) = ticker_row?;

            // Get trade stats
            let (trade_count, trade_start, trade_end) = self.conn.query_row(
                "SELECT COUNT(*),
                        MIN(timestamp),
                        MAX(timestamp)
                 FROM trades WHERE ticker_id = ?",
                [ticker_id],
                |row| {
                    let count: i64 = row.get(0)?;
                    let start: Option<i64> = row.get(1).ok();
                    let end: Option<i64> = row.get(2).ok();
                    Ok((count, start, end))
                }
            ).unwrap_or((0, None, None));

            // Convert timestamps to dates
            let trade_start_date = trade_start.and_then(|ts| {
                chrono::DateTime::from_timestamp_millis(ts)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
            });

            let trade_end_date = trade_end.and_then(|ts| {
                chrono::DateTime::from_timestamp_millis(ts)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
            });

            // Get kline stats by interval
            // Try both column names for compatibility (flowsurface uses candle_time, data-loader uses open_time)
            let kline_query = if self.conn.prepare("SELECT candle_time FROM klines LIMIT 1").is_ok() {
                "SELECT timeframe, COUNT(*), MIN(candle_time), MAX(candle_time)
                 FROM klines
                 WHERE ticker_id = ?
                 GROUP BY timeframe
                 ORDER BY timeframe"
            } else {
                "SELECT timeframe, COUNT(*), MIN(open_time), MAX(open_time)
                 FROM klines
                 WHERE ticker_id = ?
                 GROUP BY timeframe
                 ORDER BY timeframe"
            };

            let mut kline_stmt = self.conn.prepare(kline_query)?;

            let kline_intervals: Vec<KlineIntervalStats> = kline_stmt.query_map([ticker_id], |row| {
                let interval: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                let start: Option<i64> = row.get(2).ok();
                let end: Option<i64> = row.get(3).ok();

                let start_date = start.and_then(|ts| {
                    chrono::DateTime::from_timestamp_millis(ts)
                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                });

                let end_date = end.and_then(|ts| {
                    chrono::DateTime::from_timestamp_millis(ts)
                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                });

                Ok(KlineIntervalStats {
                    interval,
                    count,
                    start_date,
                    end_date,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

            stats.push(TickerStats {
                symbol,
                exchange,
                trade_count,
                trade_start_date,
                trade_end_date,
                kline_intervals,
            });
        }

        Ok(stats)
    }

    /// Get statistics for a specific ticker
    pub fn get_ticker_stats(&self, ticker_symbol: &str) -> Result<Option<TickerStats>> {
        let all_stats = self.get_all_ticker_stats()?;
        Ok(all_stats.into_iter().find(|s| s.symbol == ticker_symbol))
    }

    /// Get mutable reference to connection for import operations
    pub fn get_connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
