//! Helper functions for data import operations
//!
//! Provides utility functions for ID generation and ticker management during import.

use anyhow::Result;
use duckdb::Connection;

/// Generate a unique trade ID for insertion
///
/// Uses a simple counter-based approach for deterministic IDs
pub fn generate_trade_id(ticker_id: i64, trade_time: u64, counter: u64) -> i64 {
    // Combine ticker_id, trade_time, and counter to create unique ID
    // This ensures idempotent inserts - same data produces same ID
    ((ticker_id as i64) << 48) | ((trade_time >> 20) as i64) << 16 | ((counter & 0xFFFF) as i64)
}

/// Generate a unique kline ID for insertion
///
/// Uses ticker_id, timeframe, and open_time to create deterministic IDs
pub fn generate_kline_id(ticker_id: i64, timeframe: &str, open_time: u64, counter: u64) -> i64 {
    // Create a simple hash from timeframe string
    let timeframe_hash = timeframe
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
        .abs() & 0xFF;

    // Combine ticker_id, timeframe_hash, open_time, and counter to create unique ID
    ((ticker_id as i64) << 48) | (timeframe_hash << 40) | ((open_time >> 24) as i64) << 16 | ((counter & 0xFFFF) as i64)
}

/// Get exchange_id from exchange name, creating if needed
pub fn get_or_create_exchange_id(conn: &mut Connection, exchange_name: &str) -> Result<i64> {
    // Try to get existing exchange_id
    let maybe_id: Option<i64> = conn
        .query_row(
            "SELECT exchange_id FROM exchanges WHERE name = ?",
            [exchange_name],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = maybe_id {
        return Ok(id);
    }

    // Generate exchange_id from exchange name (simple hash constrained to TINYINT range)
    let exchange_id: i64 = (exchange_name
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
        .abs() % 127) + 1; // Keep in range 1-127 for TINYINT

    // Create new exchange record with explicit ID
    conn.execute(
        "INSERT OR IGNORE INTO exchanges (exchange_id, name) VALUES (?, ?)",
        duckdb::params![exchange_id, exchange_name],
    )?;

    Ok(exchange_id)
}

/// Resolve symbol and exchange to ticker_id, creating ticker record if needed
///
/// Handles exchange_id lookup/creation as well
pub fn get_or_create_ticker_id(
    conn: &mut Connection,
    symbol: &str,
    exchange_name: &str,
    tick_size: f32,
    min_quantity: f32,
    contract_size: Option<f32>,
) -> Result<i64> {
    // Get or create exchange_id
    let exchange_id = get_or_create_exchange_id(conn, exchange_name)?;

    // Try to get existing ticker_id
    let maybe_id: Option<i64> = conn
        .query_row(
            "SELECT ticker_id FROM tickers WHERE exchange_id = ? AND symbol = ?",
            duckdb::params![exchange_id, symbol],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = maybe_id {
        return Ok(id);
    }

    // Generate ticker_id (flowsurface doesn't use auto-increment)
    let next_id: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(ticker_id), 0) + 1 FROM tickers",
            [],
            |row| row.get(0),
        )?;

    // Create new ticker record
    // Use flowsurface-binance column names: min_ticksize, min_qty (matching the actual database schema)
    conn.execute(
        "INSERT INTO tickers (ticker_id, exchange_id, symbol, min_ticksize, min_qty, contract_size)
         VALUES (?, ?, ?, ?, ?, ?)",
        duckdb::params![
            next_id,
            exchange_id,
            symbol,
            tick_size,
            min_quantity,
            contract_size
        ],
    )?;

    Ok(next_id)
}

/// Look up existing ticker_id without creating new record
///
/// Returns error if ticker doesn't exist
#[allow(dead_code)]
fn get_ticker_id(conn: &mut Connection, symbol: &str, exchange_name: &str) -> Result<i64> {
    let id: i64 = conn
        .query_row(
            "SELECT t.ticker_id
             FROM tickers t
             JOIN exchanges e ON t.exchange_id = e.exchange_id
             WHERE e.name = ? AND t.symbol = ?",
            [exchange_name, symbol],
            |row| row.get(0),
        )
        .map_err(|e| {
            anyhow::anyhow!(
                "Ticker {}:{} not found in database: {}",
                exchange_name,
                symbol,
                e
            )
        })?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trade_id_deterministic() {
        let id1 = generate_trade_id(1, 1000, 0);
        let id2 = generate_trade_id(1, 1000, 0);
        assert_eq!(id1, id2, "Same inputs should produce same ID");
    }
}
