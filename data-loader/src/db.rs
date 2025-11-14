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

    /// Get mutable reference to connection for import operations
    pub fn get_connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
