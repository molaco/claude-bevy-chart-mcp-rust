//! ChartDatabase - DuckDB database wrapper for loading candles.

use bevy::prelude::Resource;
use duckdb::{params, Connection};
use std::sync::{Arc, Mutex};
use crate::domain::Candle;

/// Database connection resource (wrapped in Arc<Mutex> for Send + Sync).
///
/// This resource provides thread-safe access to the DuckDB database
/// for loading candlestick data.
#[derive(Resource, Clone)]
pub struct ChartDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl ChartDatabase {
    /// Create a new database connection.
    pub fn new(db_path: &str) -> Result<Self, duckdb::Error> {
        let conn = Connection::open(db_path)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Load candles from database for the given time range.
    ///
    /// Returns candles ordered by time (ascending).
    pub fn load_candles(
        &self,
        ticker_id: i32,
        timeframe: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<Candle>, duckdb::Error> {
        let query = "
            SELECT candle_time, open_price, high_price, low_price, close_price, volume
            FROM klines
            WHERE ticker_id = ? AND timeframe = ?
                AND candle_time >= ? AND candle_time <= ?
            ORDER BY candle_time ASC
        ";

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(query)?;
        let candles = stmt
            .query_map(params![ticker_id, timeframe, start_time, end_time], |row| {
                Ok(Candle {
                    time: row.get(0)?,
                    open: row.get(1)?,
                    high: row.get(2)?,
                    low: row.get(3)?,
                    close: row.get(4)?,
                    volume: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(candles)
    }

    /// Get total candle count for ticker/timeframe.
    pub fn get_candle_count(
        &self,
        ticker_id: i32,
        timeframe: &str,
    ) -> Result<usize, duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM klines WHERE ticker_id = ? AND timeframe = ?")?;
        let count: i64 = stmt.query_row(params![ticker_id, timeframe], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get time range for ticker/timeframe.
    ///
    /// Returns (min_time, max_time) as Unix timestamps in milliseconds.
    pub fn get_time_range(
        &self,
        ticker_id: i32,
        timeframe: &str,
    ) -> Result<(i64, i64), duckdb::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT MIN(candle_time), MAX(candle_time) FROM klines WHERE ticker_id = ? AND timeframe = ?",
        )?;
        let (min_time, max_time): (i64, i64) = stmt
            .query_row(params![ticker_id, timeframe], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        Ok((min_time, max_time))
    }
}
