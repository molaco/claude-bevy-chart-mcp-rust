use crate::types::Candle;
use super::types::{AggregationLevel, AggregatedCandles};
use std::collections::BTreeMap;

/// Aggregate source candles at specified level
pub fn aggregate_candles(
    source: &BTreeMap<i64, Candle>,
    level: AggregationLevel,
) -> AggregatedCandles {
    let ratio = level.ratio();
    let candle_vec: Vec<&Candle> = source.values().collect();

    if ratio == 1 {
        // No aggregation needed
        return AggregatedCandles {
            level,
            candles: candle_vec.iter().map(|c| (*c).clone()).collect(),
            source_range: (0, source.len()),
        };
    }

    let aggregated = candle_vec
        .chunks(ratio)
        .map(|chunk| aggregate_chunk_refs(chunk))
        .collect();

    AggregatedCandles {
        level,
        candles: aggregated,
        source_range: (0, source.len()),
    }
}

/// Aggregate a chunk of candles into one synthetic candle
fn aggregate_chunk(chunk: &[Candle]) -> Candle {
    assert!(!chunk.is_empty(), "Cannot aggregate empty chunk");

    let first = &chunk[0];
    let last = &chunk[chunk.len() - 1];

    let high = chunk.iter()
        .map(|c| c.high)
        .fold(f64::MIN, f64::max);

    let low = chunk.iter()
        .map(|c| c.low)
        .fold(f64::MAX, f64::min);

    let volume = chunk.iter()
        .map(|c| c.volume)
        .sum();

    Candle {
        time: first.time,
        open: first.open,
        high,
        low,
        close: last.close,
        volume,
    }
}

/// Aggregate a chunk of candle references into one synthetic candle
fn aggregate_chunk_refs(chunk: &[&Candle]) -> Candle {
    assert!(!chunk.is_empty(), "Cannot aggregate empty chunk");

    let first = chunk[0];
    let last = chunk[chunk.len() - 1];

    let high = chunk.iter()
        .map(|c| c.high)
        .fold(f64::MIN, f64::max);

    let low = chunk.iter()
        .map(|c| c.low)
        .fold(f64::MAX, f64::min);

    let volume = chunk.iter()
        .map(|c| c.volume)
        .sum();

    Candle {
        time: first.time,
        open: first.open,
        high,
        low,
        close: last.close,
        volume,
    }
}

/// Aggregate a range of source candles
/// Uses globally aligned boundaries to ensure stable aggregation during panning
pub fn aggregate_range(
    source: &BTreeMap<i64, Candle>,
    start: usize,
    count: usize,
    level: AggregationLevel,
) -> AggregatedCandles {
    let ratio = level.ratio();
    let source_len = source.len();

    if ratio == 1 {
        // No aggregation - return as-is
        let end = (start + count).min(source_len);
        let candles: Vec<Candle> = source.values()
            .skip(start)
            .take(end - start)
            .cloned()
            .collect();
        return AggregatedCandles {
            level,
            candles,
            source_range: (start, end),
        };
    }

    // Align to global boundaries to ensure consistent grouping regardless of pan position
    // Example: with ratio=10, start=1003 aligns to 1000
    // This ensures candles [1000-1009] always aggregate together
    let aligned_start = (start / ratio) * ratio;

    // Calculate end of visible range
    let end = (start + count).min(source_len);

    // Align end boundary up to cover all visible candles
    let aligned_end = ((end + ratio - 1) / ratio) * ratio;

    // Aggregate from aligned boundaries
    let slice_end = aligned_end.min(source_len);

    // Create a temporary BTreeMap with the slice of candles for aggregation
    let slice_candles: BTreeMap<i64, Candle> = source.values()
        .skip(aligned_start)
        .take(slice_end - aligned_start)
        .map(|c| (c.time, c.clone()))
        .collect();

    let mut result = aggregate_candles(&slice_candles, level);
    result.source_range = (aligned_start, slice_end);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles(count: usize) -> BTreeMap<i64, Candle> {
        (0..count)
            .map(|i| {
                let candle = Candle {
                    time: i as i64 * 1000,
                    open: 100.0 + i as f64,
                    high: 105.0 + i as f64,
                    low: 95.0 + i as f64,
                    close: 102.0 + i as f64,
                    volume: 1000.0 + i as f64,
                };
                (candle.time, candle)
            })
            .collect()
    }

    #[test]
    fn test_aggregate_level_none() {
        let candles = create_test_candles(10);
        let result = aggregate_candles(&candles, AggregationLevel::None);
        assert_eq!(result.candles.len(), 10);
        assert_eq!(result.level, AggregationLevel::None);
    }

    #[test]
    fn test_aggregate_level_low() {
        let candles = create_test_candles(10);
        let result = aggregate_candles(&candles, AggregationLevel::Low);
        assert_eq!(result.candles.len(), 5); // 10 / 2 = 5
    }

    #[test]
    fn test_aggregate_chunk_ohlc() {
        let candles = vec![
            Candle { time: 0, open: 100.0, high: 105.0, low: 95.0, close: 102.0, volume: 1000.0 },
            Candle { time: 1000, open: 102.0, high: 110.0, low: 98.0, close: 108.0, volume: 1500.0 },
            Candle { time: 2000, open: 108.0, high: 112.0, low: 106.0, close: 110.0, volume: 2000.0 },
        ];

        let result = aggregate_chunk(&candles);

        assert_eq!(result.time, 0); // First candle's time
        assert_eq!(result.open, 100.0); // First candle's open
        assert_eq!(result.high, 112.0); // Max of all highs
        assert_eq!(result.low, 95.0); // Min of all lows
        assert_eq!(result.close, 110.0); // Last candle's close
        assert_eq!(result.volume, 4500.0); // Sum of volumes
    }

    #[test]
    fn test_aggregate_partial_chunk() {
        // Test with 11 candles at 1:5 ratio (should have 2 full chunks + 1 partial)
        let candles = create_test_candles(11);
        let result = aggregate_candles(&candles, AggregationLevel::Medium); // 1:5
        assert_eq!(result.candles.len(), 3); // ceil(11/5) = 3
    }

    #[test]
    fn test_aggregate_range() {
        let candles = create_test_candles(100);
        let result = aggregate_range(&candles, 10, 50, AggregationLevel::Medium);

        assert_eq!(result.source_range, (10, 60));
        assert_eq!(result.candles.len(), 10); // 50 / 5 = 10
    }

    #[test]
    fn test_aggregation_level_select() {
        assert_eq!(AggregationLevel::select(1000, 1500), AggregationLevel::None);
        assert_eq!(AggregationLevel::select(3000, 1500), AggregationLevel::Low);
        assert_eq!(AggregationLevel::select(7500, 1500), AggregationLevel::Medium);
        assert_eq!(AggregationLevel::select(15000, 1500), AggregationLevel::High);
        assert_eq!(AggregationLevel::select(30000, 1500), AggregationLevel::VeryHigh);
        assert_eq!(AggregationLevel::select(75000, 1500), AggregationLevel::Extreme);
        assert_eq!(AggregationLevel::select(150000, 1500), AggregationLevel::Maximum);
    }

    #[test]
    #[should_panic(expected = "Cannot aggregate empty chunk")]
    fn test_aggregate_empty_chunk() {
        aggregate_chunk(&[]);
    }

    #[test]
    fn test_aggregate_all_levels() {
        let candles = create_test_candles(1000);

        let test_cases = vec![
            (AggregationLevel::None, 1000),
            (AggregationLevel::Low, 500),
            (AggregationLevel::Medium, 200),
            (AggregationLevel::High, 100),
            (AggregationLevel::VeryHigh, 50),
            (AggregationLevel::Extreme, 20),
            (AggregationLevel::Maximum, 10),
        ];

        for (level, expected_count) in test_cases {
            let result = aggregate_candles(&candles, level);
            assert_eq!(result.candles.len(), expected_count, "Failed for level {:?}", level);
            assert_eq!(result.level, level);
            assert_eq!(result.source_range, (0, 1000));
        }
    }

    #[test]
    fn test_aggregate_preserves_price_extremes() {
        // Create candles with known min/max values
        let candles = vec![
            Candle { time: 0, open: 100.0, high: 150.0, low: 50.0, close: 120.0, volume: 1000.0 },
            Candle { time: 1000, open: 120.0, high: 200.0, low: 80.0, close: 180.0, volume: 1500.0 },
            Candle { time: 2000, open: 180.0, high: 190.0, low: 30.0, close: 170.0, volume: 2000.0 },
        ];

        let result = aggregate_chunk(&candles);

        // Aggregated candle should capture the absolute highest and lowest
        assert_eq!(result.high, 200.0); // Max of all highs
        assert_eq!(result.low, 30.0);   // Min of all lows
    }

    #[test]
    fn test_aggregate_time_ordering() {
        let candles = create_test_candles(20);
        let result = aggregate_candles(&candles, AggregationLevel::Medium); // 1:5

        // Each aggregated candle should have the time of the first candle in its chunk
        assert_eq!(result.candles[0].time, 0);
        assert_eq!(result.candles[1].time, 5000);
        assert_eq!(result.candles[2].time, 10000);
        assert_eq!(result.candles[3].time, 15000);
    }

    #[test]
    fn test_aggregate_volume_summation() {
        let candles = vec![
            Candle { time: 0, open: 100.0, high: 105.0, low: 95.0, close: 102.0, volume: 1000.0 },
            Candle { time: 1000, open: 102.0, high: 110.0, low: 98.0, close: 108.0, volume: 2000.0 },
            Candle { time: 2000, open: 108.0, high: 112.0, low: 106.0, close: 110.0, volume: 3000.0 },
            Candle { time: 3000, open: 110.0, high: 115.0, low: 107.0, close: 112.0, volume: 4000.0 },
            Candle { time: 4000, open: 112.0, high: 120.0, low: 110.0, close: 118.0, volume: 5000.0 },
        ];

        let result = aggregate_chunk(&candles);
        assert_eq!(result.volume, 15000.0); // Sum of all volumes
    }

    #[test]
    fn test_aggregate_single_candle_chunk() {
        let candles = vec![
            Candle { time: 0, open: 100.0, high: 105.0, low: 95.0, close: 102.0, volume: 1000.0 },
        ];

        let result = aggregate_chunk(&candles);

        // Should return identical values
        assert_eq!(result.time, 0);
        assert_eq!(result.open, 100.0);
        assert_eq!(result.high, 105.0);
        assert_eq!(result.low, 95.0);
        assert_eq!(result.close, 102.0);
        assert_eq!(result.volume, 1000.0);
    }

    #[test]
    fn test_aggregate_range_boundary() {
        let candles = create_test_candles(100);

        // Test at the end of the data
        let result = aggregate_range(&candles, 90, 20, AggregationLevel::Medium);
        assert_eq!(result.source_range, (90, 100)); // Should clamp to array bounds
        assert_eq!(result.candles.len(), 2); // 10 candles / 5 = 2
    }

    #[test]
    fn test_aggregate_level_ratio() {
        assert_eq!(AggregationLevel::None.ratio(), 1);
        assert_eq!(AggregationLevel::Low.ratio(), 2);
        assert_eq!(AggregationLevel::Medium.ratio(), 5);
        assert_eq!(AggregationLevel::High.ratio(), 10);
        assert_eq!(AggregationLevel::VeryHigh.ratio(), 20);
        assert_eq!(AggregationLevel::Extreme.ratio(), 50);
        assert_eq!(AggregationLevel::Maximum.ratio(), 100);
    }
}
