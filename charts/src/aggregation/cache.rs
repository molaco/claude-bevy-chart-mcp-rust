use crate::types::Candle;
use super::types::{AggregationLevel, AggregatedCandles};
use super::engine::aggregate_time_range;
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;
use bevy::prelude::*;

/// Cache key for aggregated data
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub timeframe: String,
    pub level: AggregationLevel,
    pub time_start: i64,  // Was: start (usize)
    pub time_end: i64,    // Was: count (usize)
}

impl CacheKey {
    pub fn new(timeframe: String, level: AggregationLevel, time_start: i64, time_end: i64) -> Self {
        Self {
            timeframe,
            level,
            time_start,
            time_end,
        }
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Multi-level LRU cache for aggregated candle data
#[derive(Resource)]
pub struct AggregationCache {
    /// Cached aggregated candles keyed by cache key
    cache: HashMap<CacheKey, AggregatedCandles>,

    /// LRU tracking - timestamp of last access for each key
    access_times: HashMap<CacheKey, Instant>,

    /// Maximum cache size in bytes (approximate)
    max_size_bytes: usize,

    /// Current estimated cache size in bytes
    current_size_bytes: usize,

    /// Statistics
    stats: CacheStats,
}

impl Default for AggregationCache {
    fn default() -> Self {
        Self::new(50 * 1024 * 1024) // 50MB default
    }
}

impl AggregationCache {
    /// Create new cache with specified size limit in bytes
    pub fn new(max_size_bytes: usize) -> Self {
        Self {
            cache: HashMap::new(),
            access_times: HashMap::new(),
            max_size_bytes,
            current_size_bytes: 0,
            stats: CacheStats::default(),
        }
    }

    /// Get or compute aggregated candles
    pub fn get_or_aggregate(
        &mut self,
        timeframe: &str,
        source: &BTreeMap<i64, Candle>,
        time_start: i64,
        time_end: i64,
        level: AggregationLevel,
    ) -> AggregatedCandles {
        let key = CacheKey::new(timeframe.to_string(), level, time_start, time_end);

        // Check cache
        if let Some(cached) = self.cache.get(&key) {
            self.stats.hits += 1;
            self.access_times.insert(key, Instant::now());
            return cached.clone();
        }

        // Cache miss - compute aggregation
        self.stats.misses += 1;
        let aggregated = aggregate_time_range(source, time_start, time_end, level);

        // Estimate size of this entry (~56 bytes per candle)
        let entry_size = aggregated.candles.len() * 56;

        // Evict if needed before inserting
        while self.current_size_bytes + entry_size > self.max_size_bytes && !self.cache.is_empty() {
            self.evict_lru();
        }

        // Insert into cache
        self.current_size_bytes += entry_size;
        self.access_times.insert(key.clone(), Instant::now());
        self.cache.insert(key, aggregated.clone());

        self.stats.entries = self.cache.len();
        self.stats.size_bytes = self.current_size_bytes;

        aggregated
    }

    /// Evict least recently used entry
    fn evict_lru(&mut self) {
        if let Some((oldest_key, _)) = self.access_times
            .iter()
            .min_by_key(|(_, time)| *time)
        {
            let key_to_remove = oldest_key.clone();

            if let Some(removed) = self.cache.remove(&key_to_remove) {
                let entry_size = removed.candles.len() * 56;
                self.current_size_bytes = self.current_size_bytes.saturating_sub(entry_size);
                self.stats.evictions += 1;
            }

            self.access_times.remove(&key_to_remove);
            self.stats.entries = self.cache.len();
            self.stats.size_bytes = self.current_size_bytes;
        }
    }

    /// Clear entire cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_times.clear();
        self.current_size_bytes = 0;
        self.stats.entries = 0;
        self.stats.size_bytes = 0;
    }

    /// Clear cache entries for a specific timeframe
    pub fn clear_timeframe(&mut self, timeframe: &str) {
        let keys_to_remove: Vec<CacheKey> = self.cache
            .keys()
            .filter(|k| k.timeframe == timeframe)
            .cloned()
            .collect();

        for key in keys_to_remove {
            if let Some(removed) = self.cache.remove(&key) {
                let entry_size = removed.candles.len() * 56;
                self.current_size_bytes = self.current_size_bytes.saturating_sub(entry_size);
            }
            self.access_times.remove(&key);
        }

        self.stats.entries = self.cache.len();
        self.stats.size_bytes = self.current_size_bytes;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Get current cache size in bytes
    pub fn size_bytes(&self) -> usize {
        self.current_size_bytes
    }

    /// Get number of cached entries
    pub fn entry_count(&self) -> usize {
        self.cache.len()
    }
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
    fn test_cache_hit_and_miss() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024); // 10MB
        let candles = create_test_candles(1000);

        // First access - should be a miss (timestamps: 0 to 999000 with 1000ms interval)
        let _result1 = cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 1);

        // Second access with same parameters - should be a hit
        let _result2 = cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);

        // Different level - should be a miss
        let _result3 = cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Low);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = AggregationCache::new(500); // Very small cache
        let candles = create_test_candles(100);

        // Fill cache with different levels (timestamps: 0 to 99000 with 1000ms interval)
        cache.get_or_aggregate("5m", &candles, 0, 99000, AggregationLevel::Low);
        std::thread::sleep(std::time::Duration::from_millis(10));

        cache.get_or_aggregate("5m", &candles, 0, 99000, AggregationLevel::Medium);
        std::thread::sleep(std::time::Duration::from_millis(10));

        cache.get_or_aggregate("5m", &candles, 0, 99000, AggregationLevel::High);

        // Access the first entry again to make it recent
        cache.get_or_aggregate("5m", &candles, 0, 99000, AggregationLevel::Low);

        // Add a new entry - should evict Medium (least recently used)
        std::thread::sleep(std::time::Duration::from_millis(10));
        cache.get_or_aggregate("5m", &candles, 0, 99000, AggregationLevel::VeryHigh);

        // Verify eviction happened
        assert!(cache.stats().evictions > 0);
    }

    #[test]
    fn test_timeframe_specific_clearing() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024);
        let candles = create_test_candles(1000);

        // Add entries for different timeframes (timestamps: 0 to 999000 with 1000ms interval)
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        cache.get_or_aggregate("1h", &candles, 0, 999000, AggregationLevel::Medium);
        cache.get_or_aggregate("5m", &candles, 0, 499000, AggregationLevel::Low);

        assert_eq!(cache.entry_count(), 3);

        // Clear only 5m timeframe
        cache.clear_timeframe("5m");

        assert_eq!(cache.entry_count(), 1); // Only 1h should remain

        // Verify 1h is still there
        let _result = cache.get_or_aggregate("1h", &candles, 0, 999000, AggregationLevel::Medium);
        assert!(cache.stats().hits > 0); // Should be a hit
    }

    #[test]
    fn test_size_limit_enforcement() {
        let mut cache = AggregationCache::new(5000); // 5KB limit
        let candles = create_test_candles(1000);

        // Add multiple entries (timestamps: each 100 candles = 100000ms range)
        for i in 0..10 {
            let time_start = i as i64 * 100000;
            let time_end = time_start + 99000;
            cache.get_or_aggregate("5m", &candles, time_start, time_end, AggregationLevel::Medium);
        }

        // Cache size should not exceed limit (with some margin for eviction timing)
        assert!(cache.size_bytes() <= 6000); // Allow some overhead
    }

    #[test]
    fn test_statistics_tracking() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024);
        let candles = create_test_candles(1000);

        // Generate some hits and misses (timestamps: 0 to 999000 with 1000ms interval)
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Low);
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Low);
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::High);

        let stats = cache.stats();
        assert_eq!(stats.hits, 2); // Low and Medium were hit once each
        assert_eq!(stats.misses, 3); // Medium, Low, High were missed once each
        assert_eq!(stats.entries, 3); // Three different levels cached
        assert!(stats.size_bytes > 0);

        let hit_rate = stats.hit_rate();
        assert!((hit_rate - 0.4).abs() < 0.01); // 2/5 = 0.4
    }

    #[test]
    fn test_clear_all() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024);
        let candles = create_test_candles(1000);

        // Add some entries (timestamps: 0 to 999000 with 1000ms interval)
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        cache.get_or_aggregate("1h", &candles, 0, 999000, AggregationLevel::Low);

        assert_eq!(cache.entry_count(), 2);
        assert!(cache.size_bytes() > 0);

        // Clear all
        cache.clear();

        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.size_bytes(), 0);
        assert_eq!(cache.stats().entries, 0);
        assert_eq!(cache.stats().size_bytes, 0);
    }

    #[test]
    fn test_different_ranges_cached_separately() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024);
        let candles = create_test_candles(1000);

        // Same level, different time ranges (timestamps with 1000ms interval)
        cache.get_or_aggregate("5m", &candles, 0, 499000, AggregationLevel::Medium);
        cache.get_or_aggregate("5m", &candles, 500000, 999000, AggregationLevel::Medium);

        assert_eq!(cache.entry_count(), 2); // Should cache separately
    }

    #[test]
    fn test_aggregation_correctness() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024);
        let candles = create_test_candles(10);

        // Test with timestamps (10 candles: 0 to 9000 with 1000ms interval)
        let result = cache.get_or_aggregate("5m", &candles, 0, 9000, AggregationLevel::Medium);

        // Should aggregate 10 candles at 1:5 ratio
        assert_eq!(result.candles.len(), 2); // 10 / 5 = 2
        assert_eq!(result.level, AggregationLevel::Medium);
        // time_range is now in timestamps (aligned boundaries may vary)
        assert_eq!(result.time_range.0, 0); // Start aligned to 0
    }

    #[test]
    fn test_cache_persistence_across_accesses() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024);
        let candles = create_test_candles(100);

        // Test with timestamps (100 candles: 0 to 99000 with 1000ms interval)
        let result1 = cache.get_or_aggregate("5m", &candles, 0, 99000, AggregationLevel::Low);
        let result2 = cache.get_or_aggregate("5m", &candles, 0, 99000, AggregationLevel::Low);

        // Results should be identical
        assert_eq!(result1.candles.len(), result2.candles.len());
        assert_eq!(result1.level, result2.level);

        // First should be miss, second should be hit
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 1);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut cache = AggregationCache::new(10 * 1024 * 1024);
        let candles = create_test_candles(1000);

        // 1 miss (timestamps: 0 to 999000 with 1000ms interval)
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        assert_eq!(cache.stats().hit_rate(), 0.0);

        // 1 hit
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        assert_eq!(cache.stats().hit_rate(), 0.5); // 1/2

        // 2 more hits
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        cache.get_or_aggregate("5m", &candles, 0, 999000, AggregationLevel::Medium);
        assert_eq!(cache.stats().hit_rate(), 0.75); // 3/4
    }
}
