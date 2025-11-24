use crate::types::Candle;
use super::types::{AggregationLevel, AggregatedCandles};
use std::collections::BTreeMap;

/// Aggregate source candles at specified level using time-based bucketing
pub fn aggregate_candles(
    source: &BTreeMap<i64, Candle>,
    level: AggregationLevel,
) -> AggregatedCandles {
    let ratio = level.ratio();

    // Get time bounds for the time_range field
    let time_start = source.keys().next().copied().unwrap_or(0);
    let time_end = source.keys().next_back().copied().unwrap_or(0);

    if ratio == 1 {
        // No aggregation needed
        return AggregatedCandles {
            level,
            candles: source.values().cloned().collect(),
            time_range: (time_start, time_end),
        };
    }

    // Get timeframe interval for bucket calculation
    let interval_ms = infer_interval(source);
    let bucket_size = interval_ms * ratio as i64;

    // Group candles by time bucket (not by position!)
    let mut buckets: BTreeMap<i64, Vec<&Candle>> = BTreeMap::new();

    for (ts, candle) in source.iter() {
        // Calculate bucket key - aligns timestamp to bucket boundary
        let bucket_key = (*ts / bucket_size) * bucket_size;
        buckets.entry(bucket_key).or_default().push(candle);
    }

    // Aggregate each bucket
    let aggregated: Vec<Candle> = buckets
        .values()
        .filter(|bucket| !bucket.is_empty())
        .map(|bucket| aggregate_chunk_refs(bucket))
        .collect();

    AggregatedCandles {
        level,
        candles: aggregated,
        time_range: (time_start, time_end),
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

/// Align timestamp down to nearest boundary
fn align_time_down(time: i64, interval: i64) -> i64 {
    (time / interval) * interval
}

/// Align timestamp up to nearest boundary
fn align_time_up(time: i64, interval: i64) -> i64 {
    ((time + interval - 1) / interval) * interval
}

/// Infer interval from consecutive candles
fn infer_interval(source: &BTreeMap<i64, Candle>) -> i64 {
    let mut iter = source.keys();
    if let (Some(&first), Some(&second)) = (iter.next(), iter.next()) {
        second - first
    } else {
        60 * 60 * 1000 // Default 1h
    }
}

/// Aggregate candles within a time range using time-based bucketing
pub fn aggregate_time_range(
    source: &BTreeMap<i64, Candle>,
    time_start: i64,
    time_end: i64,
    level: AggregationLevel,
) -> AggregatedCandles {
    let ratio = level.ratio();

    if ratio == 1 {
        // No aggregation - return candles in range
        let candles: Vec<Candle> = source
            .range(time_start..=time_end)
            .map(|(_, c)| c.clone())
            .collect();

        return AggregatedCandles {
            level,
            candles,
            time_range: (time_start, time_end),
        };
    }

    // Get timeframe interval for bucket calculation
    let interval_ms = infer_interval(source);
    let bucket_size = interval_ms * ratio as i64;

    // Align to aggregation boundaries
    let aligned_start = align_time_down(time_start, bucket_size);
    let aligned_end = align_time_up(time_end, bucket_size);

    // Group candles by time bucket (not by position!)
    let mut buckets: BTreeMap<i64, Vec<&Candle>> = BTreeMap::new();

    for (ts, candle) in source.range(aligned_start..=aligned_end) {
        // Calculate bucket key - aligns timestamp to bucket boundary
        let bucket_key = (*ts / bucket_size) * bucket_size;
        buckets.entry(bucket_key).or_default().push(candle);
    }

    // Aggregate each bucket
    let aggregated: Vec<Candle> = buckets
        .values()
        .filter(|bucket| !bucket.is_empty())
        .map(|bucket| aggregate_chunk_refs(bucket))
        .collect();

    AggregatedCandles {
        level,
        candles: aggregated,
        time_range: (aligned_start, aligned_end),
    }
}

/// Aggregate a range of source candles (legacy index-based, kept for compatibility)
/// Uses globally aligned boundaries to ensure stable aggregation during panning
#[allow(dead_code)]
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

        // Calculate time range from the actual candles
        let time_start = candles.first().map(|c| c.time).unwrap_or(0);
        let time_end = candles.last().map(|c| c.time).unwrap_or(0);

        return AggregatedCandles {
            level,
            candles,
            time_range: (time_start, time_end),
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

    // Update time_range based on aligned boundaries
    let time_start = slice_candles.keys().next().copied().unwrap_or(0);
    let time_end = slice_candles.keys().next_back().copied().unwrap_or(0);
    result.time_range = (time_start, time_end);

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

        // time_range is now in timestamps (each candle is 1000ms apart)
        assert_eq!(result.time_range, (10000, 59000));
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
            // time_range is now in timestamps (first=0, last=999000)
            assert_eq!(result.time_range, (0, 999000));
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
        // time_range is now in timestamps (each candle is 1000ms apart)
        assert_eq!(result.time_range, (90000, 99000)); // Should clamp to array bounds
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

    #[test]
    fn test_aggregate_with_gaps_time_based() {
        // Create candles with a gap in the middle (simulating market closed periods)
        // Timestamps: 0, 1000, 2000, 5000, 6000, 7000 (gap between 2000 and 5000)
        let mut candles = BTreeMap::new();
        candles.insert(0, Candle { time: 0, open: 100.0, high: 105.0, low: 95.0, close: 102.0, volume: 1000.0 });
        candles.insert(1000, Candle { time: 1000, open: 102.0, high: 108.0, low: 100.0, close: 106.0, volume: 1100.0 });
        candles.insert(2000, Candle { time: 2000, open: 106.0, high: 110.0, low: 104.0, close: 108.0, volume: 1200.0 });
        // Gap here - no candles at 3000 or 4000
        candles.insert(5000, Candle { time: 5000, open: 120.0, high: 125.0, low: 118.0, close: 122.0, volume: 2000.0 });
        candles.insert(6000, Candle { time: 6000, open: 122.0, high: 128.0, low: 120.0, close: 126.0, volume: 2100.0 });
        candles.insert(7000, Candle { time: 7000, open: 126.0, high: 130.0, low: 124.0, close: 128.0, volume: 2200.0 });

        // With Low level (ratio=2, bucket_size=2000), we should get:
        // Bucket 0: candles at 0, 1000 -> aggregated
        // Bucket 2000: candle at 2000 (only one, but still valid bucket)
        // Bucket 4000: candle at 5000 (5000/2000*2000 = 4000) -> only one candle
        // Bucket 6000: candles at 6000, 7000 -> aggregated
        let result = aggregate_candles(&candles, AggregationLevel::Low);

        // Should have 4 buckets, not 3 (which would happen with position-based chunking)
        assert_eq!(result.candles.len(), 4);

        // Verify first bucket contains candles 0 and 1000
        assert_eq!(result.candles[0].time, 0);
        assert_eq!(result.candles[0].open, 100.0); // First candle's open
        assert_eq!(result.candles[0].close, 106.0); // Last candle in bucket's close

        // Verify second bucket contains only candle 2000
        assert_eq!(result.candles[1].time, 2000);
        assert_eq!(result.candles[1].open, 106.0);
        assert_eq!(result.candles[1].close, 108.0);

        // Verify third bucket contains candle 5000 (bucket key 4000)
        assert_eq!(result.candles[2].time, 5000);

        // Verify fourth bucket contains candles 6000 and 7000
        assert_eq!(result.candles[3].time, 6000);
        assert_eq!(result.candles[3].close, 128.0); // Last candle's close
    }

    #[test]
    fn test_aggregate_time_range_with_gaps() {
        // Same gap scenario but using aggregate_time_range
        let mut candles = BTreeMap::new();
        candles.insert(0, Candle { time: 0, open: 100.0, high: 105.0, low: 95.0, close: 102.0, volume: 1000.0 });
        candles.insert(1000, Candle { time: 1000, open: 102.0, high: 108.0, low: 100.0, close: 106.0, volume: 1100.0 });
        candles.insert(2000, Candle { time: 2000, open: 106.0, high: 110.0, low: 104.0, close: 108.0, volume: 1200.0 });
        // Gap
        candles.insert(5000, Candle { time: 5000, open: 120.0, high: 125.0, low: 118.0, close: 122.0, volume: 2000.0 });
        candles.insert(6000, Candle { time: 6000, open: 122.0, high: 128.0, low: 120.0, close: 126.0, volume: 2100.0 });
        candles.insert(7000, Candle { time: 7000, open: 126.0, high: 130.0, low: 124.0, close: 128.0, volume: 2200.0 });

        let result = aggregate_time_range(&candles, 0, 7000, AggregationLevel::Low);

        // Time-based bucketing should produce 4 aggregated candles
        assert_eq!(result.candles.len(), 4);
    }

    #[test]
    fn test_panning_stability() {
        // Verify that different pan positions produce consistent aggregations
        // This was the main bug: panning caused candles to re-group differently
        let candles = create_test_candles(100);

        // Query overlapping ranges
        // With Medium level (ratio=5) and interval=1000ms, bucket_size = 5000ms
        let result1 = aggregate_time_range(&candles, 0, 50000, AggregationLevel::Medium);
        let result2 = aggregate_time_range(&candles, 10000, 60000, AggregationLevel::Medium);

        // The overlapping region should have identical aggregations for COMPLETE buckets
        // Buckets at range edges may be partial, so we compare interior buckets only
        // Interior buckets: 10000, 15000, 20000, 25000, 30000, 35000, 40000, 45000
        // (excluding 50000 which is partial in result1)
        let overlap1: Vec<_> = result1.candles.iter()
            .filter(|c| c.time >= 10000 && c.time < 50000)
            .collect();
        let overlap2: Vec<_> = result2.candles.iter()
            .filter(|c| c.time >= 10000 && c.time < 50000)
            .collect();

        assert_eq!(overlap1.len(), overlap2.len(), "Same number of complete buckets in overlap");

        // Same timestamps in overlapping region should have same OHLC values
        for (c1, c2) in overlap1.iter().zip(overlap2.iter()) {
            assert_eq!(c1.time, c2.time, "Timestamps should match in overlapping region");
            assert_eq!(c1.open, c2.open, "Open prices should match");
            assert_eq!(c1.high, c2.high, "High prices should match");
            assert_eq!(c1.low, c2.low, "Low prices should match");
            assert_eq!(c1.close, c2.close, "Close prices should match");
        }
    }

    #[test]
    fn test_bucket_boundaries_globally_consistent() {
        // Verify that the same candle always ends up in the same bucket
        // regardless of query range
        let candles = create_test_candles(100);

        // Query different ranges that all include candle at timestamp 25000
        let result1 = aggregate_time_range(&candles, 0, 30000, AggregationLevel::Medium);
        let result2 = aggregate_time_range(&candles, 20000, 50000, AggregationLevel::Medium);
        let result3 = aggregate_time_range(&candles, 25000, 35000, AggregationLevel::Medium);

        // All should have a bucket starting at 25000
        let bucket1 = result1.candles.iter().find(|c| c.time == 25000);
        let bucket2 = result2.candles.iter().find(|c| c.time == 25000);
        let bucket3 = result3.candles.iter().find(|c| c.time == 25000);

        // All three queries should produce the same bucket for timestamp 25000
        assert!(bucket1.is_some(), "Bucket 25000 should exist in result1");
        assert!(bucket2.is_some(), "Bucket 25000 should exist in result2");
        assert!(bucket3.is_some(), "Bucket 25000 should exist in result3");

        let b1 = bucket1.unwrap();
        let b2 = bucket2.unwrap();
        let b3 = bucket3.unwrap();

        // OHLC should be identical
        assert_eq!(b1.open, b2.open);
        assert_eq!(b1.open, b3.open);
        assert_eq!(b1.high, b2.high);
        assert_eq!(b1.high, b3.high);
        assert_eq!(b1.low, b2.low);
        assert_eq!(b1.low, b3.low);
        assert_eq!(b1.close, b2.close);
        assert_eq!(b1.close, b3.close);
    }
}
