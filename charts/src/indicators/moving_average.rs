//! Moving Average indicator implementation.
//!
//! Provides Simple Moving Average (SMA) and Exponential Moving Average (EMA)
//! calculations with support for incremental updates during lazy loading.

use bevy::prelude::Color;
use crate::domain::Candle;

/// Moving Average indicator.
///
/// Supports both SMA and EMA calculations with efficient incremental
/// updates for append (scroll right) and prepend (scroll left) operations.
#[derive(Debug, Clone)]
pub struct MovingAverage {
    /// Period for the moving average (e.g., 20, 50, 200)
    pub period: usize,
    /// Cached MA values per candle (None if not enough data)
    pub values: Vec<Option<f32>>,
    /// Display name (e.g., "SMA-20", "EMA-50")
    pub name: String,
    /// Line color for rendering
    pub color: Color,
    /// Toggle visibility in chart
    pub visible: bool,
}

impl MovingAverage {
    /// Calculate Simple Moving Average using sliding window (O(n) complexity).
    ///
    /// Returns a vector of Option<f32> where indices before `period - 1`
    /// are None (not enough data), and subsequent indices contain the SMA value.
    pub fn calculate_sma(candles: &[Candle], period: usize) -> Vec<Option<f32>> {
        let mut values = vec![None; candles.len()];

        if candles.len() < period {
            return values;
        }

        // Calculate initial sum for first window [0..period-1]
        let mut sum: f64 = candles[0..period]
            .iter()
            .map(|c| c.close)
            .sum();

        // Store first SMA value
        values[period - 1] = Some((sum / period as f64) as f32);

        // Sliding window: for each subsequent candle
        // Remove oldest value, add newest value
        for i in period..candles.len() {
            sum -= candles[i - period].close;  // Remove value leaving window
            sum += candles[i].close;           // Add value entering window
            values[i] = Some((sum / period as f64) as f32);
        }

        values
    }

    /// Calculate Simple Moving Average (NAIVE - for testing only).
    ///
    /// This O(n*k) implementation is used to verify the sliding window algorithm.
    #[cfg(test)]
    pub fn calculate_sma_naive(candles: &[Candle], period: usize) -> Vec<Option<f32>> {
        let mut values = vec![None; candles.len()];

        if candles.len() < period {
            return values;
        }

        for i in (period - 1)..candles.len() {
            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();
            values[i] = Some((sum / period as f64) as f32);
        }

        values
    }

    /// Calculate SMA for NEW candles being appended (scrolling right).
    ///
    /// Only calculates values starting from `old_len` index, reusing
    /// existing calculations for efficiency.
    pub fn calculate_sma_append(&mut self, candles: &[Candle], old_len: usize) {
        let period = self.period;

        // Ensure values vec is sized correctly
        if self.values.len() < old_len {
            self.values.resize(old_len, None);
        }

        // Calculate MA for new candles starting at old_len
        for i in old_len..candles.len() {
            if i < period - 1 {
                // Not enough data for MA yet
                self.values.push(None);
                continue;
            }

            // Use sliding window if we have a previous MA value
            if i >= period && i > 0 && self.values.len() > i - 1 {
                if let Some(prev_ma) = self.values[i - 1] {
                    // Reconstruct sum from previous MA
                    let mut sum = prev_ma as f64 * period as f64;

                    // Sliding window: remove oldest, add newest
                    sum -= candles[i - period].close;
                    sum += candles[i].close;

                    self.values.push(Some((sum / period as f64) as f32));
                    continue;
                }
            }

            // Fallback: calculate from scratch for this value
            // (happens at boundary between old and new data)
            if i >= period - 1 {
                let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                    .iter()
                    .map(|c| c.close)
                    .sum();
                self.values.push(Some((sum / period as f64) as f32));
            } else {
                self.values.push(None);
            }
        }
    }

    /// Calculate SMA for NEW candles being prepended (scrolling left into history).
    ///
    /// Calculates values for first `new_count` candles, then prepends to existing values.
    pub fn calculate_sma_prepend(&mut self, candles: &[Candle], new_count: usize) {
        let period = self.period;
        let mut new_values = Vec::with_capacity(candles.len());

        // Calculate MA for new candles at the beginning
        for i in 0..new_count {
            if i < period - 1 {
                new_values.push(None);
                continue;
            }

            // Calculate from scratch (can't use sliding window going backwards)
            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();
            new_values.push(Some((sum / period as f64) as f32));
        }

        // Recalculate boundary values that now include prepended candles
        // Need to recalculate up to (period - 1) values after the prepended section
        let boundary_end = (new_count + period - 1).min(candles.len());
        for i in new_count..boundary_end {
            if i < period - 1 {
                new_values.push(None);
                continue;
            }

            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();
            new_values.push(Some((sum / period as f64) as f32));
        }

        // Append remaining old values that don't need recalculation
        if boundary_end < candles.len() {
            let remaining_old_values_start = boundary_end - new_count;
            new_values.extend_from_slice(&self.values[remaining_old_values_start..]);
        }

        self.values = new_values;
    }

    /// Recalculate a specific range of MA values (for boundary corrections).
    pub fn recalculate_range(&mut self, candles: &[Candle], start: usize, end: usize) {
        let period = self.period;
        let end = end.min(candles.len());

        for i in start..end {
            if i < period - 1 {
                if i < self.values.len() {
                    self.values[i] = None;
                }
                continue;
            }

            let sum: f64 = candles[i.saturating_sub(period - 1)..=i]
                .iter()
                .map(|c| c.close)
                .sum();

            let value = Some((sum / period as f64) as f32);

            if i < self.values.len() {
                self.values[i] = value;
            } else {
                self.values.push(value);
            }
        }
    }

    /// Calculate Exponential Moving Average.
    ///
    /// The first value is calculated as SMA, then subsequent values use
    /// exponential smoothing with multiplier = 2 / (period + 1).
    pub fn calculate_ema(candles: &[Candle], period: usize) -> Vec<Option<f32>> {
        let mut values = vec![None; candles.len()];

        if candles.len() < period {
            return values;
        }

        // First value is SMA
        let sma: f64 = candles[0..period]
            .iter()
            .map(|c| c.close)
            .sum::<f64>() / period as f64;
        values[period - 1] = Some(sma as f32);

        // Subsequent values use exponential smoothing
        let multiplier = 2.0 / (period as f64 + 1.0);
        for i in period..candles.len() {
            let prev = values[i - 1].unwrap_or(sma as f32);
            let ema = (candles[i].close as f32 - prev) * multiplier as f32 + prev;
            values[i] = Some(ema);
        }

        values
    }

    /// Create a new SMA indicator.
    pub fn new_sma(candles: &[Candle], period: usize, color: Color) -> Self {
        Self {
            period,
            values: Self::calculate_sma(candles, period),
            name: format!("SMA-{}", period),
            color,
            visible: true,
        }
    }

    /// Create a new EMA indicator.
    pub fn new_ema(candles: &[Candle], period: usize, color: Color) -> Self {
        Self {
            period,
            values: Self::calculate_ema(candles, period),
            name: format!("EMA-{}", period),
            color,
            visible: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles(count: usize) -> Vec<Candle> {
        (0..count)
            .map(|i| Candle {
                time: i as i64 * 1000,
                open: (i as f64) * 10.0,
                high: (i as f64) * 10.0 + 5.0,
                low: (i as f64) * 10.0 - 5.0,
                close: (i as f64) * 10.0 + 1.0,
                volume: 1000.0,
            })
            .collect()
    }

    #[test]
    fn test_sma_sliding_window_matches_naive() {
        let candles = create_test_candles(1000);

        let sma_new = MovingAverage::calculate_sma(&candles, 20);
        let sma_old = MovingAverage::calculate_sma_naive(&candles, 20);

        assert_eq!(sma_new.len(), sma_old.len());

        for i in 0..candles.len() {
            match (sma_new[i], sma_old[i]) {
                (Some(a), Some(b)) => {
                    let diff = (a - b).abs();
                    assert!(
                        diff < 0.001,
                        "Mismatch at index {}: new={}, old={}, diff={}",
                        i, a, b, diff
                    );
                }
                (None, None) => {}
                _ => panic!("Option mismatch at index {}: new={:?}, old={:?}", i, sma_new[i], sma_old[i]),
            }
        }
    }

    #[test]
    fn test_sma_different_periods() {
        let candles = create_test_candles(500);

        for period in [5, 10, 20, 50, 100, 200] {
            let sma_new = MovingAverage::calculate_sma(&candles, period);
            let sma_old = MovingAverage::calculate_sma_naive(&candles, period);

            for i in 0..candles.len() {
                match (sma_new[i], sma_old[i]) {
                    (Some(a), Some(b)) => {
                        assert!(
                            (a - b).abs() < 0.001,
                            "Period {} mismatch at index {}: {} vs {}",
                            period, i, a, b
                        );
                    }
                    (None, None) => {}
                    _ => panic!("Period {} option mismatch at index {}", period, i),
                }
            }
        }
    }

    #[test]
    fn test_sma_edge_case_empty_candles() {
        let candles: Vec<Candle> = vec![];
        let result = MovingAverage::calculate_sma(&candles, 20);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_sma_edge_case_insufficient_candles() {
        let candles = create_test_candles(10);
        let result = MovingAverage::calculate_sma(&candles, 20);
        assert_eq!(result.len(), 10);
        assert!(result.iter().all(|v| v.is_none()));
    }

    #[test]
    fn test_sma_edge_case_exact_period() {
        let candles = create_test_candles(20);
        let result = MovingAverage::calculate_sma(&candles, 20);

        // First 19 should be None
        for i in 0..19 {
            assert!(result[i].is_none(), "Index {} should be None", i);
        }

        // Index 19 should have a value
        assert!(result[19].is_some(), "Index 19 should have a value");
    }

    #[test]
    fn test_sma_edge_case_period_one() {
        let candles = create_test_candles(10);
        let result = MovingAverage::calculate_sma(&candles, 1);

        // All values should equal close prices
        for i in 0..candles.len() {
            match result[i] {
                Some(v) => {
                    assert_eq!(v, candles[i].close as f32);
                }
                None => panic!("Expected value at index {}", i),
            }
        }
    }

    #[test]
    fn test_sma_numerical_stability() {
        // Test with large numbers to check floating point stability
        let candles: Vec<Candle> = (0..100)
            .map(|i| Candle {
                time: i,
                open: 1_000_000.0 + i as f64,
                high: 1_000_000.0 + i as f64,
                low: 1_000_000.0 + i as f64,
                close: 1_000_000.0 + i as f64,
                volume: 1000.0,
            })
            .collect();

        let sma_new = MovingAverage::calculate_sma(&candles, 20);
        let sma_old = MovingAverage::calculate_sma_naive(&candles, 20);

        for i in 20..candles.len() {
            if let (Some(a), Some(b)) = (sma_new[i], sma_old[i]) {
                let relative_error = ((a - b) / b).abs();
                assert!(
                    relative_error < 0.00001,
                    "Numerical instability at {}: {} vs {}, error: {}",
                    i, a, b, relative_error
                );
            }
        }
    }

    #[test]
    fn test_sma_append_incremental() {
        // Create initial dataset
        let mut candles = create_test_candles(1000);

        // Calculate full SMA
        let mut ma_incremental = MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 1.0, 1.0));
        let full_reference = ma_incremental.values.clone();

        // Add more candles
        let old_len = candles.len();
        candles.extend(create_test_candles(100).iter().map(|c| Candle {
            time: c.time + old_len as i64 * 1000,
            open: c.open + old_len as f64 * 10.0,
            high: c.high + old_len as f64 * 10.0,
            low: c.low + old_len as f64 * 10.0,
            close: c.close + old_len as f64 * 10.0,
            volume: c.volume,
        }));

        // Calculate incrementally
        ma_incremental.calculate_sma_append(&candles, old_len);

        // Calculate from scratch for comparison
        let ma_full = MovingAverage::calculate_sma(&candles, 20);

        // Verify old values unchanged
        for i in 0..old_len {
            match (ma_incremental.values[i], full_reference[i]) {
                (Some(a), Some(b)) => {
                    assert_eq!(a, b, "Old value changed at index {}", i);
                }
                (None, None) => {}
                _ => panic!("Old value changed at index {}", i),
            }
        }

        // Verify new values match full calculation
        for i in old_len..candles.len() {
            match (ma_incremental.values[i], ma_full[i]) {
                (Some(a), Some(b)) => {
                    assert!(
                        (a - b).abs() < 0.001,
                        "New value mismatch at index {}: {} vs {}",
                        i, a, b
                    );
                }
                (None, None) => {}
                _ => panic!("New value mismatch at index {}", i),
            }
        }
    }

    #[test]
    fn test_sma_prepend_incremental() {
        // Create initial dataset starting at index 100
        let candles_old = create_test_candles(100)
            .iter()
            .map(|c| Candle {
                time: c.time + 100_000,
                open: c.open + 1000.0,
                high: c.high + 1000.0,
                low: c.low + 1000.0,
                close: c.close + 1000.0,
                volume: c.volume,
            })
            .collect::<Vec<_>>();

        // Calculate SMA for old data
        let mut ma_incremental = MovingAverage::new_sma(&candles_old, 20, Color::srgb(1.0, 1.0, 1.0));

        // Create full dataset (new + old)
        let mut candles_full = create_test_candles(100);
        candles_full.extend(candles_old);

        // Calculate incrementally (prepend)
        ma_incremental.calculate_sma_prepend(&candles_full, 100);

        // Calculate from scratch for comparison
        let ma_full = MovingAverage::calculate_sma(&candles_full, 20);

        // Verify entire result matches
        assert_eq!(ma_incremental.values.len(), ma_full.len());

        for i in 0..candles_full.len() {
            match (ma_incremental.values[i], ma_full[i]) {
                (Some(a), Some(b)) => {
                    assert!(
                        (a - b).abs() < 0.001,
                        "Mismatch at index {}: {} vs {}",
                        i, a, b
                    );
                }
                (None, None) => {}
                _ => panic!("Option mismatch at index {}", i),
            }
        }
    }

    #[test]
    fn test_sma_append_boundary_conditions() {
        let mut candles = create_test_candles(50);
        let mut ma = MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 1.0, 1.0));

        // Append just 1 candle
        let old_len = candles.len();
        candles.push(Candle {
            time: 50000,
            open: 500.0,
            high: 505.0,
            low: 495.0,
            close: 501.0,
            volume: 1000.0,
        });

        ma.calculate_sma_append(&candles, old_len);

        // Should have exactly one new value
        assert_eq!(ma.values.len(), 51);
        assert!(ma.values[50].is_some());
    }

    #[test]
    fn test_sma_prepend_insufficient_period() {
        // Create candles where new data doesn't have enough for period
        let candles_old = create_test_candles(100);
        let mut ma = MovingAverage::new_sma(&candles_old, 50, Color::srgb(1.0, 1.0, 1.0));

        // Prepend only 10 candles (not enough for period=50)
        let mut candles_full = create_test_candles(10);
        candles_full.extend(candles_old);

        ma.calculate_sma_prepend(&candles_full, 10);

        // First 10 values should be None (not enough for period)
        for i in 0..10 {
            assert!(ma.values[i].is_none(), "Index {} should be None", i);
        }
    }

    #[test]
    fn test_sma_append_different_periods() {
        // Test incremental append with various periods
        for period in [20, 50, 100] {
            let mut candles = create_test_candles(500);
            let mut ma = MovingAverage::new_sma(&candles, period, Color::srgb(1.0, 1.0, 1.0));

            let old_len = candles.len();
            candles.extend(create_test_candles(100));

            ma.calculate_sma_append(&candles, old_len);
            let ma_full = MovingAverage::calculate_sma(&candles, period);

            for i in old_len..candles.len() {
                match (ma.values[i], ma_full[i]) {
                    (Some(a), Some(b)) => {
                        assert!(
                            (a - b).abs() < 0.001,
                            "Period {} mismatch at {}: {} vs {}",
                            period, i, a, b
                        );
                    }
                    (None, None) => {}
                    _ => panic!("Period {} option mismatch at {}", period, i),
                }
            }
        }
    }
}
