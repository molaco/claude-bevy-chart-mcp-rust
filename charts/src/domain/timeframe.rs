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

    // ========================================================================
    // ADDITIONAL TIMEFRAME TESTS
    // ========================================================================

    #[test]
    fn test_timeframe_to_seconds() {
        assert_eq!(Timeframe::M1.to_seconds(), 60);
        assert_eq!(Timeframe::M5.to_seconds(), 5 * 60);
        assert_eq!(Timeframe::M15.to_seconds(), 15 * 60);
        assert_eq!(Timeframe::M30.to_seconds(), 30 * 60);
        assert_eq!(Timeframe::H1.to_seconds(), 3600);
        assert_eq!(Timeframe::H4.to_seconds(), 4 * 3600);
        assert_eq!(Timeframe::D1.to_seconds(), 86400);
        assert_eq!(Timeframe::W1.to_seconds(), 7 * 86400);
        assert_eq!(Timeframe::MN1.to_seconds(), 30 * 86400);
    }

    #[test]
    fn test_timeframe_all() {
        let all = Timeframe::all();

        assert_eq!(all.len(), 9);
        assert!(all.contains(&Timeframe::M1));
        assert!(all.contains(&Timeframe::M5));
        assert!(all.contains(&Timeframe::M15));
        assert!(all.contains(&Timeframe::M30));
        assert!(all.contains(&Timeframe::H1));
        assert!(all.contains(&Timeframe::H4));
        assert!(all.contains(&Timeframe::D1));
        assert!(all.contains(&Timeframe::W1));
        assert!(all.contains(&Timeframe::MN1));
    }

    #[test]
    fn test_timeframe_description() {
        assert_eq!(Timeframe::M1.description(), "1 Minute");
        assert_eq!(Timeframe::M5.description(), "5 Minutes");
        assert_eq!(Timeframe::H1.description(), "1 Hour");
        assert_eq!(Timeframe::D1.description(), "1 Day");
        assert_eq!(Timeframe::W1.description(), "1 Week");
        assert_eq!(Timeframe::MN1.description(), "1 Month");
    }

    #[test]
    fn test_timeframe_default() {
        assert_eq!(Timeframe::default(), Timeframe::M1);
    }

    #[test]
    fn test_timeframe_parse_invalid_strings() {
        let invalid = ["", "abc", "1", "m", "h", "d", "M1", "1M "];
        for s in invalid {
            assert!(
                s.parse::<Timeframe>().is_err(),
                "'{}' should fail to parse",
                s
            );
        }
    }

    #[test]
    fn test_timeframe_as_str_all_variants() {
        assert_eq!(Timeframe::M1.as_str(), "1m");
        assert_eq!(Timeframe::M5.as_str(), "5m");
        assert_eq!(Timeframe::M15.as_str(), "15m");
        assert_eq!(Timeframe::M30.as_str(), "30m");
        assert_eq!(Timeframe::H1.as_str(), "1h");
        assert_eq!(Timeframe::H4.as_str(), "4h");
        assert_eq!(Timeframe::D1.as_str(), "1d");
        assert_eq!(Timeframe::W1.as_str(), "1w");
        assert_eq!(Timeframe::MN1.as_str(), "1M");
    }

    #[test]
    fn test_timeframe_durations_increase() {
        // Each larger timeframe should have a longer duration
        let timeframes = [
            Timeframe::M1,
            Timeframe::M5,
            Timeframe::M15,
            Timeframe::M30,
            Timeframe::H1,
            Timeframe::H4,
            Timeframe::D1,
            Timeframe::W1,
            Timeframe::MN1,
        ];

        for i in 0..(timeframes.len() - 1) {
            assert!(
                timeframes[i].to_ms() < timeframes[i + 1].to_ms(),
                "{:?} should have shorter duration than {:?}",
                timeframes[i],
                timeframes[i + 1]
            );
        }
    }

    #[test]
    fn test_timeframe_clone_and_copy() {
        let tf = Timeframe::H4;
        let cloned = tf.clone();
        let copied = tf; // Copy trait

        assert_eq!(tf, cloned);
        assert_eq!(tf, copied);
    }

    #[test]
    fn test_timeframe_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(Timeframe::M1);
        set.insert(Timeframe::H4);
        set.insert(Timeframe::M1); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&Timeframe::M1));
        assert!(set.contains(&Timeframe::H4));
    }

    #[test]
    fn test_parse_timeframe_error_display() {
        let result = "invalid".parse::<Timeframe>();
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("invalid"));
        assert!(msg.contains("1m"));
    }

    #[test]
    fn test_timeframe_ms_known_values() {
        // Verify exact millisecond values
        assert_eq!(Timeframe::M1.to_ms(), 60_000);
        assert_eq!(Timeframe::H1.to_ms(), 3_600_000);
        assert_eq!(Timeframe::D1.to_ms(), 86_400_000);
        assert_eq!(Timeframe::W1.to_ms(), 604_800_000);
    }
}
