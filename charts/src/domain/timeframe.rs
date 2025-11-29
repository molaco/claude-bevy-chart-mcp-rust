//! Timeframe representation for candlestick charts.

use std::fmt;
use std::str::FromStr;

/// Type-safe timeframe representation for candlestick data.
///
/// Replaces string-based timeframe matching with a proper enum,
/// providing compile-time safety and clear duration semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeframe {
    /// 1 minute candles
    M1,
    /// 5 minute candles
    M5,
    /// 15 minute candles
    M15,
    /// 30 minute candles
    M30,
    /// 1 hour candles
    H1,
    /// 4 hour candles
    H4,
    /// 1 day candles
    D1,
    /// 1 week candles
    W1,
    /// 1 month candles (approximated as 30 days)
    MN1,
}

impl Timeframe {
    /// Get the duration of this timeframe in milliseconds.
    pub const fn to_ms(&self) -> i64 {
        match self {
            Timeframe::M1 => 60 * 1000,
            Timeframe::M5 => 5 * 60 * 1000,
            Timeframe::M15 => 15 * 60 * 1000,
            Timeframe::M30 => 30 * 60 * 1000,
            Timeframe::H1 => 60 * 60 * 1000,
            Timeframe::H4 => 4 * 60 * 60 * 1000,
            Timeframe::D1 => 24 * 60 * 60 * 1000,
            Timeframe::W1 => 7 * 24 * 60 * 60 * 1000,
            Timeframe::MN1 => 30 * 24 * 60 * 60 * 1000,
        }
    }

    /// Get the duration of this timeframe in seconds.
    pub const fn to_seconds(&self) -> i64 {
        self.to_ms() / 1000
    }

    /// Get the canonical string representation (for database queries).
    pub const fn as_str(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1w",
            Timeframe::MN1 => "1M",
        }
    }

    /// Get all supported timeframes.
    pub const fn all() -> &'static [Timeframe] {
        &[
            Timeframe::M1,
            Timeframe::M5,
            Timeframe::M15,
            Timeframe::M30,
            Timeframe::H1,
            Timeframe::H4,
            Timeframe::D1,
            Timeframe::W1,
            Timeframe::MN1,
        ]
    }

    /// Get a human-readable description.
    pub const fn description(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1 Minute",
            Timeframe::M5 => "5 Minutes",
            Timeframe::M15 => "15 Minutes",
            Timeframe::M30 => "30 Minutes",
            Timeframe::H1 => "1 Hour",
            Timeframe::H4 => "4 Hours",
            Timeframe::D1 => "1 Day",
            Timeframe::W1 => "1 Week",
            Timeframe::MN1 => "1 Month",
        }
    }
}

/// Error type for timeframe parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTimeframeError {
    input: String,
}

impl fmt::Display for ParseTimeframeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown timeframe '{}', valid options: 1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M",
            self.input
        )
    }
}

impl std::error::Error for ParseTimeframeError {}

impl FromStr for Timeframe {
    type Err = ParseTimeframeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1m" => Ok(Timeframe::M1),
            "5m" => Ok(Timeframe::M5),
            "15m" => Ok(Timeframe::M15),
            "30m" => Ok(Timeframe::M30),
            "1h" => Ok(Timeframe::H1),
            "4h" => Ok(Timeframe::H4),
            "1d" => Ok(Timeframe::D1),
            "1w" => Ok(Timeframe::W1),
            "1M" => Ok(Timeframe::MN1),
            _ => Err(ParseTimeframeError {
                input: s.to_string(),
            }),
        }
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for Timeframe {
    fn default() -> Self {
        Timeframe::M1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_to_ms() {
        assert_eq!(Timeframe::M1.to_ms(), 60_000);
        assert_eq!(Timeframe::M15.to_ms(), 15 * 60_000);
        assert_eq!(Timeframe::H1.to_ms(), 3_600_000);
        assert_eq!(Timeframe::H4.to_ms(), 4 * 3_600_000);
        assert_eq!(Timeframe::D1.to_ms(), 86_400_000);
    }

    #[test]
    fn test_timeframe_from_str() {
        assert_eq!("1m".parse::<Timeframe>().unwrap(), Timeframe::M1);
        assert_eq!("15m".parse::<Timeframe>().unwrap(), Timeframe::M15);
        assert_eq!("1h".parse::<Timeframe>().unwrap(), Timeframe::H1);
        assert_eq!("4h".parse::<Timeframe>().unwrap(), Timeframe::H4);
        assert_eq!("1d".parse::<Timeframe>().unwrap(), Timeframe::D1);
        assert_eq!("1M".parse::<Timeframe>().unwrap(), Timeframe::MN1);

        assert!("invalid".parse::<Timeframe>().is_err());
    }

    #[test]
    fn test_timeframe_roundtrip() {
        for tf in Timeframe::all() {
            let s = tf.as_str();
            let parsed: Timeframe = s.parse().unwrap();
            assert_eq!(*tf, parsed);
        }
    }

    #[test]
    fn test_timeframe_display() {
        assert_eq!(format!("{}", Timeframe::M1), "1m");
        assert_eq!(format!("{}", Timeframe::H4), "4h");
        assert_eq!(format!("{}", Timeframe::D1), "1d");
    }
}
