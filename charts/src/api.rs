//! Binance REST API client for fetching historical klines (candlestick data).
//!
//! This module provides async functions for fetching OHLCV data from Binance's
//! public REST API.

use crate::error::{ChartError, Result};
use crate::types::Candle;

/// Base URL for Binance klines endpoint
pub const BINANCE_API_BASE: &str = "https://api.binance.com/api/v3/klines";

/// Maps timeframe strings to Binance interval format.
///
/// Supports both lowercase (1m, 5m, etc.) and uppercase (M1, M5, etc.) formats.
///
/// # Arguments
/// * `timeframe` - Timeframe string in either format
///
/// # Returns
/// * `Ok(&'static str)` - The Binance-compatible interval string
/// * `Err(ChartError)` - If the timeframe is not supported
///
/// # Examples
/// ```
/// use charts::api::map_timeframe;
///
/// assert_eq!(map_timeframe("1m").unwrap(), "1m");
/// assert_eq!(map_timeframe("M1").unwrap(), "1m");
/// assert_eq!(map_timeframe("4h").unwrap(), "4h");
/// assert_eq!(map_timeframe("H4").unwrap(), "4h");
/// ```
pub fn map_timeframe(timeframe: &str) -> Result<&'static str> {
    match timeframe {
        // Lowercase variants (native Binance format)
        "1m" => Ok("1m"),
        "5m" => Ok("5m"),
        "15m" => Ok("15m"),
        "30m" => Ok("30m"),
        "1h" => Ok("1h"),
        "4h" => Ok("4h"),
        "1d" => Ok("1d"),
        // Uppercase variants (legacy format)
        "M1" => Ok("1m"),
        "M5" => Ok("5m"),
        "M15" => Ok("15m"),
        "M30" => Ok("30m"),
        "H1" => Ok("1h"),
        "H4" => Ok("4h"),
        "D1" => Ok("1d"),
        _ => Err(ChartError::invalid_timeframe(format!(
            "Unsupported timeframe: {}. Supported: 1m, 5m, 15m, 30m, 1h, 4h, 1d (or M1, M5, M15, M30, H1, H4, D1)",
            timeframe
        ))),
    }
}

/// Fetches historical klines from Binance.
///
/// # Arguments
/// * `ticker` - Trading pair symbol (e.g., "BTCUSDT")
/// * `timeframe` - Timeframe string: 1m, 5m, 15m, 30m, 1h, 4h, 1d (or M1, M5, etc.)
/// * `limit` - Number of klines to fetch (max 1000)
///
/// # Returns
/// A vector of Candle structs or a ChartError.
///
/// # Examples
/// ```no_run
/// use charts::api::fetch_klines;
///
/// #[tokio::main]
/// async fn main() {
///     let candles = fetch_klines("BTCUSDT", "1h", 100).await.unwrap();
///     println!("Fetched {} candles", candles.len());
/// }
/// ```
pub async fn fetch_klines(ticker: &str, timeframe: &str, limit: u32) -> Result<Vec<Candle>> {
    let interval = map_timeframe(timeframe)?;
    let limit = limit.min(1000);

    let url = format!(
        "{}?symbol={}&interval={}&limit={}",
        BINANCE_API_BASE,
        ticker.to_uppercase(),
        interval,
        limit
    );

    let response = reqwest::get(&url)
        .await
        .map_err(|e| ChartError::api_request(format!("HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(ChartError::api_request(format!(
            "API error: {}",
            response.status()
        )));
    }

    let text = response
        .text()
        .await
        .map_err(|e| ChartError::api_request(format!("Failed to read response: {}", e)))?;

    parse_klines_response(&text)
}

/// Fetches historical klines from Binance within a specific time range.
///
/// # Arguments
/// * `ticker` - Trading pair symbol (e.g., "BTCUSDT")
/// * `timeframe` - Timeframe string: 1m, 5m, 15m, 30m, 1h, 4h, 1d (or M1, M5, etc.)
/// * `start_time` - Start timestamp in milliseconds
/// * `end_time` - End timestamp in milliseconds
///
/// # Returns
/// A vector of Candle structs or a ChartError.
///
/// # Examples
/// ```no_run
/// use charts::api::fetch_klines_range;
///
/// #[tokio::main]
/// async fn main() {
///     let start = 1609459200000; // Jan 1, 2021
///     let end = 1609545600000;   // Jan 2, 2021
///     let candles = fetch_klines_range("BTCUSDT", "1h", start, end).await.unwrap();
///     println!("Fetched {} candles", candles.len());
/// }
/// ```
pub async fn fetch_klines_range(
    ticker: &str,
    timeframe: &str,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<Candle>> {
    let interval = map_timeframe(timeframe)?;

    // Calculate approximate number of klines needed based on interval
    let interval_ms = match timeframe {
        "1m" | "M1" => 60_000i64,
        "5m" | "M5" => 300_000,
        "15m" | "M15" => 900_000,
        "30m" | "M30" => 1_800_000,
        "1h" | "H1" => 3_600_000,
        "4h" | "H4" => 14_400_000,
        "1d" | "D1" => 86_400_000,
        _ => 60_000,
    };

    let time_diff = (end_time - start_time).max(0) as u64;
    let num_klines = (time_diff / interval_ms as u64).min(1000) as u32;
    let limit = num_klines.max(1);

    let url = format!(
        "{}?symbol={}&interval={}&startTime={}&endTime={}&limit={}",
        BINANCE_API_BASE,
        ticker.to_uppercase(),
        interval,
        start_time,
        end_time,
        limit
    );

    let response = reqwest::get(&url)
        .await
        .map_err(|e| ChartError::api_request(format!("HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(ChartError::api_request(format!(
            "API error: {}",
            response.status()
        )));
    }

    let text = response
        .text()
        .await
        .map_err(|e| ChartError::api_request(format!("Failed to read response: {}", e)))?;

    parse_klines_response(&text)
}

/// Parses Binance klines JSON response into Candle structs.
///
/// Binance returns klines as arrays of arrays with the following format:
/// ```json
/// [
///   [
///     1499040000000,      // 0: Open time (ms)
///     "0.01634000",       // 1: Open
///     "0.80000000",       // 2: High
///     "0.01575800",       // 3: Low
///     "0.01577100",       // 4: Close
///     "148976.11427815",  // 5: Volume
///     1499644799999,      // 6: Close time
///     "2434.19055334",    // 7: Quote asset volume
///     308,                // 8: Number of trades
///     "1756.87402397",    // 9: Taker buy base asset volume
///     "28.46694368",      // 10: Taker buy quote asset volume
///     "17928899.62484339" // 11: Ignore
///   ]
/// ]
/// ```
fn parse_klines_response(text: &str) -> Result<Vec<Candle>> {
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|e| ChartError::parse(format!("Failed to parse JSON: {}", e)))?;

    let arr = json
        .as_array()
        .ok_or_else(|| ChartError::parse("Expected JSON array".to_string()))?;

    let mut candles = Vec::with_capacity(arr.len());

    for (idx, item) in arr.iter().enumerate() {
        let kline_arr = item
            .as_array()
            .ok_or_else(|| ChartError::parse(format!("Expected kline array at index {}", idx)))?;

        if kline_arr.len() < 6 {
            return Err(ChartError::parse(format!(
                "Invalid kline data at index {}: insufficient fields (got {}, need 6)",
                idx,
                kline_arr.len()
            )));
        }

        let time = kline_arr[0]
            .as_i64()
            .ok_or_else(|| ChartError::parse(format!("Invalid open_time at index {}", idx)))?;

        let open = kline_arr[1]
            .as_str()
            .ok_or_else(|| ChartError::parse(format!("Invalid open at index {}", idx)))?
            .parse::<f64>()
            .map_err(|e| ChartError::parse(format!("Failed to parse open at index {}: {}", idx, e)))?;

        let high = kline_arr[2]
            .as_str()
            .ok_or_else(|| ChartError::parse(format!("Invalid high at index {}", idx)))?
            .parse::<f64>()
            .map_err(|e| ChartError::parse(format!("Failed to parse high at index {}: {}", idx, e)))?;

        let low = kline_arr[3]
            .as_str()
            .ok_or_else(|| ChartError::parse(format!("Invalid low at index {}", idx)))?
            .parse::<f64>()
            .map_err(|e| ChartError::parse(format!("Failed to parse low at index {}: {}", idx, e)))?;

        let close = kline_arr[4]
            .as_str()
            .ok_or_else(|| ChartError::parse(format!("Invalid close at index {}", idx)))?
            .parse::<f64>()
            .map_err(|e| ChartError::parse(format!("Failed to parse close at index {}: {}", idx, e)))?;

        let volume = kline_arr[5]
            .as_str()
            .ok_or_else(|| ChartError::parse(format!("Invalid volume at index {}", idx)))?
            .parse::<f64>()
            .map_err(|e| ChartError::parse(format!("Failed to parse volume at index {}: {}", idx, e)))?;

        candles.push(Candle {
            time,
            open,
            high,
            low,
            close,
            volume,
        });
    }

    Ok(candles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_timeframe_lowercase() {
        assert_eq!(map_timeframe("1m").unwrap(), "1m");
        assert_eq!(map_timeframe("5m").unwrap(), "5m");
        assert_eq!(map_timeframe("15m").unwrap(), "15m");
        assert_eq!(map_timeframe("30m").unwrap(), "30m");
        assert_eq!(map_timeframe("1h").unwrap(), "1h");
        assert_eq!(map_timeframe("4h").unwrap(), "4h");
        assert_eq!(map_timeframe("1d").unwrap(), "1d");
    }

    #[test]
    fn test_map_timeframe_uppercase() {
        assert_eq!(map_timeframe("M1").unwrap(), "1m");
        assert_eq!(map_timeframe("M5").unwrap(), "5m");
        assert_eq!(map_timeframe("M15").unwrap(), "15m");
        assert_eq!(map_timeframe("M30").unwrap(), "30m");
        assert_eq!(map_timeframe("H1").unwrap(), "1h");
        assert_eq!(map_timeframe("H4").unwrap(), "4h");
        assert_eq!(map_timeframe("D1").unwrap(), "1d");
    }

    #[test]
    fn test_map_timeframe_invalid() {
        assert!(map_timeframe("2h").is_err());
        assert!(map_timeframe("invalid").is_err());
        assert!(map_timeframe("").is_err());
    }

    #[test]
    fn test_parse_klines_response() {
        let json = r#"[
            [1609459200000, "29000.00", "29500.00", "28500.00", "29200.00", "1000.5", 1609462799999, "0", 0, "0", "0", "0"],
            [1609462800000, "29200.00", "29800.00", "29100.00", "29700.00", "1500.75", 1609466399999, "0", 0, "0", "0", "0"]
        ]"#;

        let candles = parse_klines_response(json).unwrap();
        assert_eq!(candles.len(), 2);

        assert_eq!(candles[0].time, 1609459200000);
        assert_eq!(candles[0].open, 29000.0);
        assert_eq!(candles[0].high, 29500.0);
        assert_eq!(candles[0].low, 28500.0);
        assert_eq!(candles[0].close, 29200.0);
        assert_eq!(candles[0].volume, 1000.5);

        assert_eq!(candles[1].time, 1609462800000);
        assert_eq!(candles[1].open, 29200.0);
        assert_eq!(candles[1].high, 29800.0);
        assert_eq!(candles[1].low, 29100.0);
        assert_eq!(candles[1].close, 29700.0);
        assert_eq!(candles[1].volume, 1500.75);
    }

    #[test]
    fn test_parse_klines_response_empty() {
        let json = "[]";
        let candles = parse_klines_response(json).unwrap();
        assert!(candles.is_empty());
    }

    #[test]
    fn test_parse_klines_response_invalid_json() {
        let json = "not json";
        assert!(parse_klines_response(json).is_err());
    }

    #[test]
    fn test_parse_klines_response_insufficient_fields() {
        let json = r#"[[1609459200000, "29000.00", "29500.00"]]"#;
        assert!(parse_klines_response(json).is_err());
    }
}
