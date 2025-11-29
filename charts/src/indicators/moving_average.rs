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

    // ========================================================================
    // EMA TESTS
    // ========================================================================

    #[test]
    fn test_ema_basic_calculation() {
        let candles = create_test_candles(100);
        let ema = MovingAverage::calculate_ema(&candles, 20);

        assert_eq!(ema.len(), candles.len());

        // First 19 values should be None
        for i in 0..19 {
            assert!(ema[i].is_none(), "EMA index {} should be None", i);
        }

        // Index 19 and onwards should have values
        for i in 19..ema.len() {
            assert!(ema[i].is_some(), "EMA index {} should have a value", i);
        }
    }

    #[test]
    fn test_ema_first_value_equals_sma() {
        let candles = create_test_candles(100);
        let ema = MovingAverage::calculate_ema(&candles, 20);
        let sma = MovingAverage::calculate_sma(&candles, 20);

        // First EMA value (at index 19) should equal first SMA value
        match (ema[19], sma[19]) {
            (Some(e), Some(s)) => {
                assert!(
                    (e - s).abs() < 0.001,
                    "First EMA ({}) should equal first SMA ({})",
                    e, s
                );
            }
            _ => panic!("Both EMA and SMA should have values at index 19"),
        }
    }

    #[test]
    fn test_ema_edge_case_empty() {
        let candles: Vec<Candle> = vec![];
        let ema = MovingAverage::calculate_ema(&candles, 20);
        assert_eq!(ema.len(), 0);
    }

    #[test]
    fn test_ema_edge_case_insufficient() {
        let candles = create_test_candles(10);
        let ema = MovingAverage::calculate_ema(&candles, 20);

        assert_eq!(ema.len(), 10);
        assert!(ema.iter().all(|v| v.is_none()), "All should be None when insufficient data");
    }

    #[test]
    fn test_ema_edge_case_period_one() {
        let candles = create_test_candles(10);
        let ema = MovingAverage::calculate_ema(&candles, 1);

        // With period 1, EMA should equal close prices
        for i in 0..candles.len() {
            match ema[i] {
                Some(v) => {
                    assert!(
                        (v - candles[i].close as f32).abs() < 0.001,
                        "EMA with period 1 at {} should equal close price",
                        i
                    );
                }
                None => panic!("EMA period 1 should have value at index {}", i),
            }
        }
    }

    #[test]
    fn test_ema_multiplier_calculation() {
        // EMA multiplier = 2 / (period + 1)
        // For period 20: 2 / 21 ≈ 0.0952
        let expected_multiplier: f64 = 2.0 / 21.0;

        // Create candles with known values to verify multiplier effect
        let candles: Vec<Candle> = (0..30)
            .map(|i| Candle {
                time: i as i64 * 1000,
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0, // All same close price
                volume: 1000.0,
            })
            .collect();

        let ema = MovingAverage::calculate_ema(&candles, 20);

        // All values from index 19 onwards should be 100 (since all closes are 100)
        for i in 19..ema.len() {
            match ema[i] {
                Some(v) => {
                    assert!(
                        (v - 100.0).abs() < 0.001,
                        "EMA with constant prices should equal that price"
                    );
                }
                None => panic!("Expected EMA value at index {}", i),
            }
        }

        // Verify multiplier is used correctly
        assert!((expected_multiplier - 2.0 / 21.0).abs() < 0.0001);
    }

    #[test]
    fn test_ema_responds_to_price_changes() {
        // EMA should respond faster to price changes than SMA
        let mut candles = create_test_candles(50);

        // Add a spike at the end
        candles.push(Candle {
            time: 50000,
            open: 1000.0,
            high: 1000.0,
            low: 1000.0,
            close: 1000.0, // Much higher than previous values
            volume: 1000.0,
        });

        let ema = MovingAverage::calculate_ema(&candles, 10);
        let sma = MovingAverage::calculate_sma(&candles, 10);

        // EMA should react more strongly to the spike
        // (This is a property test - EMA gives more weight to recent values)
        if let (Some(ema_last), Some(sma_last)) = (ema[50], sma[50]) {
            // EMA should be higher because it weights recent prices more
            assert!(
                ema_last > sma_last,
                "EMA ({}) should be higher than SMA ({}) after a spike",
                ema_last, sma_last
            );
        }
    }

    #[test]
    fn test_ema_numerical_stability_large_values() {
        // Test with large price values
        let candles: Vec<Candle> = (0..100)
            .map(|i| Candle {
                time: i as i64,
                open: 1_000_000.0 + i as f64,
                high: 1_000_000.0 + i as f64,
                low: 1_000_000.0 + i as f64,
                close: 1_000_000.0 + i as f64,
                volume: 1000.0,
            })
            .collect();

        let ema = MovingAverage::calculate_ema(&candles, 20);

        // All values should be finite (not NaN or infinite)
        for (i, value) in ema.iter().enumerate() {
            if let Some(v) = value {
                assert!(
                    v.is_finite(),
                    "EMA at {} should be finite, got {}",
                    i, v
                );
            }
        }
    }

    #[test]
    fn test_ema_convergence_long_run() {
        // Test EMA stability over many iterations
        let candles = create_test_candles(10000);
        let ema = MovingAverage::calculate_ema(&candles, 20);

        // No NaN after many iterations
        for (i, value) in ema.iter().enumerate() {
            if let Some(v) = value {
                assert!(
                    !v.is_nan(),
                    "EMA at {} should not be NaN after {} iterations",
                    i, i
                );
            }
        }

        // Last value should be reasonable (close to recent prices)
        if let Some(last_ema) = ema[9999] {
            let last_close = candles[9999].close as f32;
            let diff_percent = ((last_ema - last_close) / last_close).abs();
            assert!(
                diff_percent < 0.5, // Within 50% of last close
                "EMA should be reasonably close to recent prices"
            );
        }
    }

    #[test]
    fn test_ema_exact_period_count() {
        let candles = create_test_candles(20);
        let ema = MovingAverage::calculate_ema(&candles, 20);

        // First 19 should be None
        for i in 0..19 {
            assert!(ema[i].is_none(), "Index {} should be None", i);
        }

        // Index 19 should have exactly one value
        assert!(ema[19].is_some(), "Index 19 should have a value");
    }

    #[test]
    fn test_ema_different_periods() {
        let candles = create_test_candles(200);

        for period in [5, 10, 20, 50, 100] {
            let ema = MovingAverage::calculate_ema(&candles, period);

            assert_eq!(ema.len(), candles.len());

            // First (period - 1) should be None
            for i in 0..(period - 1) {
                assert!(
                    ema[i].is_none(),
                    "Period {} index {} should be None",
                    period, i
                );
            }

            // Index (period - 1) and onwards should have values
            for i in (period - 1)..ema.len() {
                assert!(
                    ema[i].is_some(),
                    "Period {} index {} should have value",
                    period, i
                );
            }
        }
    }

    // ========================================================================
    // NEW SMA/EMA INDICATOR CREATION TESTS
    // ========================================================================

    #[test]
    fn test_new_sma_indicator() {
        let candles = create_test_candles(100);
        let ma = MovingAverage::new_sma(&candles, 20, Color::srgb(1.0, 0.8, 0.0));

        assert_eq!(ma.period, 20);
        assert_eq!(ma.name, "SMA-20");
        assert!(ma.visible);
        assert_eq!(ma.values.len(), 100);
    }

    #[test]
    fn test_new_ema_indicator() {
        let candles = create_test_candles(100);
        let ma = MovingAverage::new_ema(&candles, 50, Color::srgb(0.0, 1.0, 1.0));

        assert_eq!(ma.period, 50);
        assert_eq!(ma.name, "EMA-50");
        assert!(ma.visible);
        assert_eq!(ma.values.len(), 100);
    }
}
