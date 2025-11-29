//! CandleData - Data store for loaded candle data.

use bevy::prelude::Resource;
use crate::domain::Candle;

/// Candle data resource - holds all loaded candle data.
///
/// This resource stores the candles that have been loaded from the database.
/// It supports lazy loading by tracking a global offset for the data window.
#[derive(Resource)]
pub struct CandleData {
    /// All loaded candles
    pub candles: Vec<Candle>,
    /// Global offset for lazy loading (tracks position in full dataset)
    pub candle_offset: usize,
}

impl Default for CandleData {
    fn default() -> Self {
        Self {
            candles: Vec::new(),
            candle_offset: 0,
        }
    }
}

impl CandleData {
    /// Create a new CandleData with the given candles.
    pub fn new(candles: Vec<Candle>) -> Self {
        Self {
            candles,
            candle_offset: 0,
        }
    }

    /// Create a new CandleData with the given candles and offset.
    pub fn with_offset(candles: Vec<Candle>, offset: usize) -> Self {
        Self {
            candles,
            candle_offset: offset,
        }
    }

    /// Get the number of loaded candles.
    pub fn len(&self) -> usize {
        self.candles.len()
    }

    /// Check if there are no loaded candles.
    pub fn is_empty(&self) -> bool {
        self.candles.is_empty()
    }

    /// Get the first candle (earliest in time).
    pub fn first(&self) -> Option<&Candle> {
        self.candles.first()
    }

    /// Get the last candle (most recent in time).
    pub fn last(&self) -> Option<&Candle> {
        self.candles.last()
    }

    /// Get the time range of loaded candles.
    pub fn time_range(&self) -> Option<(i64, i64)> {
        match (self.first(), self.last()) {
            (Some(first), Some(last)) => Some((first.time, last.time)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles(count: usize) -> Vec<Candle> {
        (0..count)
            .map(|i| Candle::new(
                (i as i64 + 1) * 1000,  // time: 1000, 2000, 3000, ...
                100.0 + i as f64,       // open
                105.0 + i as f64,       // high
                95.0 + i as f64,        // low
                102.0 + i as f64,       // close
                1000.0,                  // volume
            ))
            .collect()
    }

    // ========================================================================
    // CREATION TESTS
    // ========================================================================

    #[test]
    fn test_candle_data_default() {
        let data = CandleData::default();
        assert!(data.candles.is_empty());
        assert_eq!(data.candle_offset, 0);
    }

    #[test]
    fn test_candle_data_new() {
        let candles = create_test_candles(10);
        let data = CandleData::new(candles.clone());

        assert_eq!(data.candles.len(), 10);
        assert_eq!(data.candle_offset, 0);
    }

    #[test]
    fn test_candle_data_with_offset() {
        let candles = create_test_candles(10);
        let data = CandleData::with_offset(candles.clone(), 100);

        assert_eq!(data.candles.len(), 10);
        assert_eq!(data.candle_offset, 100);
    }

    // ========================================================================
    // len() AND is_empty() TESTS
    // ========================================================================

    #[test]
    fn test_candle_data_len() {
        let data = CandleData::new(create_test_candles(25));
        assert_eq!(data.len(), 25);
    }

    #[test]
    fn test_candle_data_len_empty() {
        let data = CandleData::default();
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn test_candle_data_is_empty_true() {
        let data = CandleData::default();
        assert!(data.is_empty());
    }

    #[test]
    fn test_candle_data_is_empty_false() {
        let data = CandleData::new(create_test_candles(1));
        assert!(!data.is_empty());
    }

    // ========================================================================
    // first() AND last() TESTS
    // ========================================================================

    #[test]
    fn test_candle_data_first() {
        let data = CandleData::new(create_test_candles(5));
        let first = data.first();

        assert!(first.is_some());
        assert_eq!(first.unwrap().time, 1000); // First candle has time=1000
    }

    #[test]
    fn test_candle_data_first_empty() {
        let data = CandleData::default();
        assert!(data.first().is_none());
    }

    #[test]
    fn test_candle_data_last() {
        let data = CandleData::new(create_test_candles(5));
        let last = data.last();

        assert!(last.is_some());
        assert_eq!(last.unwrap().time, 5000); // Last candle (index 4) has time=5000
    }

    #[test]
    fn test_candle_data_last_empty() {
        let data = CandleData::default();
        assert!(data.last().is_none());
    }

    #[test]
    fn test_candle_data_first_last_single() {
        let data = CandleData::new(create_test_candles(1));

        let first = data.first();
        let last = data.last();

        assert!(first.is_some());
        assert!(last.is_some());
        assert_eq!(first.unwrap().time, last.unwrap().time);
    }

    // ========================================================================
    // time_range() TESTS
    // ========================================================================

    #[test]
    fn test_candle_data_time_range() {
        let data = CandleData::new(create_test_candles(10));
        let range = data.time_range();

        assert!(range.is_some());
        let (min, max) = range.unwrap();
        assert_eq!(min, 1000);  // First candle
        assert_eq!(max, 10000); // Last candle (index 9)
    }

    #[test]
    fn test_candle_data_time_range_empty() {
        let data = CandleData::default();
        assert!(data.time_range().is_none());
    }

    #[test]
    fn test_candle_data_time_range_single() {
        let data = CandleData::new(create_test_candles(1));
        let range = data.time_range();

        assert!(range.is_some());
        let (min, max) = range.unwrap();
        assert_eq!(min, max);
        assert_eq!(min, 1000);
    }

    // ========================================================================
    // MUTATION TESTS
    // ========================================================================

    #[test]
    fn test_candle_data_append() {
        let mut data = CandleData::new(create_test_candles(5));
        let more_candles = create_test_candles(3);

        data.candles.extend(more_candles);

        assert_eq!(data.len(), 8);
    }

    #[test]
    fn test_candle_data_prepend() {
        let mut data = CandleData::new(create_test_candles(5));
        let earlier_candles = create_test_candles(3);

        let mut combined = earlier_candles;
        combined.append(&mut data.candles);
        data.candles = combined;

        assert_eq!(data.len(), 8);
    }

    // ========================================================================
    // BOUNDARY CONDITION TESTS
    // ========================================================================

    #[test]
    fn test_candle_data_large_count() {
        let data = CandleData::new(create_test_candles(10000));

        assert_eq!(data.len(), 10000);
        assert!(!data.is_empty());
        assert!(data.first().is_some());
        assert!(data.last().is_some());

        let range = data.time_range().unwrap();
        assert_eq!(range.0, 1000);
        assert_eq!(range.1, 10000 * 1000);
    }

    #[test]
    fn test_candle_data_offset_does_not_affect_methods() {
        let candles = create_test_candles(10);
        let data_no_offset = CandleData::new(candles.clone());
        let data_with_offset = CandleData::with_offset(candles.clone(), 5000);

        // len(), first(), last(), time_range() should be the same
        assert_eq!(data_no_offset.len(), data_with_offset.len());
        assert_eq!(data_no_offset.first().unwrap().time, data_with_offset.first().unwrap().time);
        assert_eq!(data_no_offset.last().unwrap().time, data_with_offset.last().unwrap().time);
        assert_eq!(data_no_offset.time_range(), data_with_offset.time_range());

        // Only offset differs
        assert_ne!(data_no_offset.candle_offset, data_with_offset.candle_offset);
    }
}
