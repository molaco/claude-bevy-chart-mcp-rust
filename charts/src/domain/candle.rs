//! Candle data structure representing OHLCV candlestick data.

/// Raw candle from database representing a single candlestick.
///
/// Contains Open, High, Low, Close, Volume (OHLCV) data along with
/// a timestamp. This is the fundamental data unit for candlestick charting.
#[derive(Debug, Clone, PartialEq)]
pub struct Candle {
    /// Unix timestamp in milliseconds
    pub time: i64,
    /// Opening price
    pub open: f64,
    /// Highest price during the period
    pub high: f64,
    /// Lowest price during the period
    pub low: f64,
    /// Closing price
    pub close: f64,
    /// Trading volume
    pub volume: f64,
}

impl Candle {
    /// Create a new candle with the given OHLCV data.
    pub fn new(time: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            time,
            open,
            high,
            low,
            close,
            volume,
        }
    }

    /// Check if this candle has valid OHLCV data.
    ///
    /// Validates:
    /// - High >= Low
    /// - High >= Open and High >= Close
    /// - Low <= Open and Low <= Close
    /// - Volume >= 0
    /// - All prices are finite (not NaN or infinite)
    pub fn is_valid(&self) -> bool {
        // Check for NaN or infinite values
        if !self.open.is_finite()
            || !self.high.is_finite()
            || !self.low.is_finite()
            || !self.close.is_finite()
            || !self.volume.is_finite()
        {
            return false;
        }

        // Validate OHLC relationships
        self.high >= self.low
            && self.high >= self.open
            && self.high >= self.close
            && self.low <= self.open
            && self.low <= self.close
            && self.volume >= 0.0
    }

    /// Check if this is a bullish candle (close >= open).
    #[inline]
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    /// Check if this is a bearish candle (close < open).
    #[inline]
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// Get the body size (absolute difference between open and close).
    #[inline]
    pub fn body_size(&self) -> f64 {
        (self.close - self.open).abs()
    }

    /// Get the upper wick size.
    #[inline]
    pub fn upper_wick(&self) -> f64 {
        self.high - self.open.max(self.close)
    }

    /// Get the lower wick size.
    #[inline]
    pub fn lower_wick(&self) -> f64 {
        self.open.min(self.close) - self.low
    }

    /// Get the total range (high - low).
    #[inline]
    pub fn range(&self) -> f64 {
        self.high - self.low
    }

    /// Get the midpoint price ((high + low) / 2).
    #[inline]
    pub fn midpoint(&self) -> f64 {
        (self.high + self.low) / 2.0
    }

    /// Get the typical price ((high + low + close) / 3).
    #[inline]
    pub fn typical_price(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candle_is_valid() {
        // Valid bullish candle
        let candle = Candle::new(1000, 100.0, 110.0, 95.0, 105.0, 1000.0);
        assert!(candle.is_valid());

        // Valid bearish candle
        let candle = Candle::new(1000, 105.0, 110.0, 95.0, 100.0, 1000.0);
        assert!(candle.is_valid());

        // Invalid: high < low
        let candle = Candle::new(1000, 100.0, 95.0, 110.0, 105.0, 1000.0);
        assert!(!candle.is_valid());

        // Invalid: high < open
        let candle = Candle::new(1000, 100.0, 99.0, 95.0, 98.0, 1000.0);
        assert!(!candle.is_valid());

        // Invalid: negative volume
        let candle = Candle::new(1000, 100.0, 110.0, 95.0, 105.0, -100.0);
        assert!(!candle.is_valid());

        // Invalid: NaN price
        let candle = Candle::new(1000, f64::NAN, 110.0, 95.0, 105.0, 1000.0);
        assert!(!candle.is_valid());

        // Invalid: infinite price
        let candle = Candle::new(1000, 100.0, f64::INFINITY, 95.0, 105.0, 1000.0);
        assert!(!candle.is_valid());
    }

    #[test]
    fn test_candle_bullish_bearish() {
        let bullish = Candle::new(1000, 100.0, 110.0, 95.0, 105.0, 1000.0);
        assert!(bullish.is_bullish());
        assert!(!bullish.is_bearish());

        let bearish = Candle::new(1000, 105.0, 110.0, 95.0, 100.0, 1000.0);
        assert!(bearish.is_bearish());
        assert!(!bearish.is_bullish());

        // Doji (open == close) is bullish by convention
        let doji = Candle::new(1000, 100.0, 110.0, 95.0, 100.0, 1000.0);
        assert!(doji.is_bullish());
    }

    #[test]
    fn test_candle_measurements() {
        let candle = Candle::new(1000, 100.0, 115.0, 90.0, 110.0, 1000.0);

        assert_eq!(candle.body_size(), 10.0); // |110 - 100| = 10
        assert_eq!(candle.upper_wick(), 5.0); // 115 - max(100, 110) = 115 - 110 = 5
        assert_eq!(candle.lower_wick(), 10.0); // min(100, 110) - 90 = 100 - 90 = 10
        assert_eq!(candle.range(), 25.0); // 115 - 90 = 25
        assert_eq!(candle.midpoint(), 102.5); // (115 + 90) / 2 = 102.5
        assert!((candle.typical_price() - 105.0).abs() < 0.0001); // (115 + 90 + 110) / 3 = 105
    }
}
