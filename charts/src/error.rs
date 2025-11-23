//! Custom error types for the charts library.
//!
//! This module provides a comprehensive error handling system using `thiserror`
//! for ergonomic error definitions with automatic `Display` and `Error` implementations.

use thiserror::Error;

/// The primary error type for chart operations.
///
/// This enum covers all possible error conditions that can occur during
/// chart data fetching, processing, storage, and validation.
#[derive(Debug, Error)]
pub enum ChartError {
    /// REST API request failures (connection errors, HTTP errors, rate limits, etc.)
    #[error("API request failed: {0}")]
    ApiRequest(String),

    /// WebSocket connection or message handling errors
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// JSON or data parsing errors
    #[error("Parse error: {0}")]
    Parse(String),

    /// DuckDB database errors
    #[error("Database error: {0}")]
    Database(String),

    /// Invalid timeframe input (e.g., unsupported interval strings)
    #[error("Invalid timeframe: {0}")]
    InvalidTimeframe(String),

    /// OHLC data validation errors (e.g., high < low, negative volume)
    #[error("Data validation error: {0}")]
    DataValidation(String),

    /// Standard IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A specialized `Result` type for chart operations.
pub type Result<T> = std::result::Result<T, ChartError>;

impl ChartError {
    /// Creates an `ApiRequest` error with the given message.
    ///
    /// # Examples
    ///
    /// ```
    /// use charts::error::ChartError;
    ///
    /// let err = ChartError::api_request("Connection timeout");
    /// ```
    pub fn api_request(msg: impl Into<String>) -> Self {
        Self::ApiRequest(msg.into())
    }

    /// Creates a `WebSocket` error with the given message.
    ///
    /// # Examples
    ///
    /// ```
    /// use charts::error::ChartError;
    ///
    /// let err = ChartError::websocket("Connection closed unexpectedly");
    /// ```
    pub fn websocket(msg: impl Into<String>) -> Self {
        Self::WebSocket(msg.into())
    }

    /// Creates a `Parse` error with the given message.
    ///
    /// # Examples
    ///
    /// ```
    /// use charts::error::ChartError;
    ///
    /// let err = ChartError::parse("Invalid JSON: expected object");
    /// ```
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    /// Creates a `Database` error with the given message.
    ///
    /// # Examples
    ///
    /// ```
    /// use charts::error::ChartError;
    ///
    /// let err = ChartError::database("Query execution failed");
    /// ```
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Creates an `InvalidTimeframe` error with the given message.
    ///
    /// # Examples
    ///
    /// ```
    /// use charts::error::ChartError;
    ///
    /// let err = ChartError::invalid_timeframe("'2h' is not a supported timeframe");
    /// ```
    pub fn invalid_timeframe(msg: impl Into<String>) -> Self {
        Self::InvalidTimeframe(msg.into())
    }

    /// Creates a `DataValidation` error with the given message.
    ///
    /// # Examples
    ///
    /// ```
    /// use charts::error::ChartError;
    ///
    /// let err = ChartError::data_validation("High price cannot be less than low price");
    /// ```
    pub fn data_validation(msg: impl Into<String>) -> Self {
        Self::DataValidation(msg.into())
    }

    /// Returns `true` if this is a retryable error.
    ///
    /// Retryable errors include transient API failures and WebSocket disconnections.
    /// Non-retryable errors include validation errors and parse errors.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::ApiRequest(_) | Self::WebSocket(_) | Self::Io(_))
    }

    /// Returns `true` if this is a validation-related error.
    pub fn is_validation_error(&self) -> bool {
        matches!(self, Self::DataValidation(_) | Self::InvalidTimeframe(_))
    }

    /// Returns `true` if this is a data/parsing-related error.
    pub fn is_data_error(&self) -> bool {
        matches!(self, Self::Parse(_) | Self::DataValidation(_))
    }

    /// Returns `true` if this is a connection-related error.
    pub fn is_connection_error(&self) -> bool {
        matches!(self, Self::ApiRequest(_) | Self::WebSocket(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ChartError::api_request("timeout");
        assert_eq!(err.to_string(), "API request failed: timeout");

        let err = ChartError::websocket("disconnected");
        assert_eq!(err.to_string(), "WebSocket error: disconnected");

        let err = ChartError::parse("invalid json");
        assert_eq!(err.to_string(), "Parse error: invalid json");

        let err = ChartError::database("connection failed");
        assert_eq!(err.to_string(), "Database error: connection failed");

        let err = ChartError::invalid_timeframe("2h");
        assert_eq!(err.to_string(), "Invalid timeframe: 2h");

        let err = ChartError::data_validation("high < low");
        assert_eq!(err.to_string(), "Data validation error: high < low");
    }

    #[test]
    fn test_is_retryable() {
        assert!(ChartError::api_request("timeout").is_retryable());
        assert!(ChartError::websocket("disconnected").is_retryable());
        assert!(!ChartError::parse("invalid").is_retryable());
        assert!(!ChartError::database("error").is_retryable());
        assert!(!ChartError::invalid_timeframe("2h").is_retryable());
        assert!(!ChartError::data_validation("invalid").is_retryable());
    }

    #[test]
    fn test_is_validation_error() {
        assert!(ChartError::invalid_timeframe("2h").is_validation_error());
        assert!(ChartError::data_validation("invalid").is_validation_error());
        assert!(!ChartError::api_request("timeout").is_validation_error());
        assert!(!ChartError::parse("invalid").is_validation_error());
    }

    #[test]
    fn test_is_data_error() {
        assert!(ChartError::parse("invalid").is_data_error());
        assert!(ChartError::data_validation("invalid").is_data_error());
        assert!(!ChartError::api_request("timeout").is_data_error());
        assert!(!ChartError::websocket("disconnected").is_data_error());
    }

    #[test]
    fn test_is_connection_error() {
        assert!(ChartError::api_request("timeout").is_connection_error());
        assert!(ChartError::websocket("disconnected").is_connection_error());
        assert!(!ChartError::parse("invalid").is_connection_error());
        assert!(!ChartError::database("error").is_connection_error());
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let chart_err: ChartError = io_err.into();
        assert!(matches!(chart_err, ChartError::Io(_)));
        assert!(chart_err.is_retryable());
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_ok() -> Result<i32> {
            Ok(42)
        }

        fn returns_err() -> Result<i32> {
            Err(ChartError::parse("test"))
        }

        assert!(returns_ok().is_ok());
        assert!(returns_err().is_err());
    }
}
