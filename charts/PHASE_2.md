# Phase 2: Incremental MA Recalculation on Lazy Load

**Priority:** High
**Risk Level:** Medium
**Estimated Time:** 3-4 hours
**Dependencies:** Phase 1 (Sliding Window Algorithm)

---

## Overview

When lazy loading 100 new candles, only recalculate MA values for those 100 candles instead of recalculating all 10,000+ existing values. This builds on Phase 1's optimized sliding window algorithm.

### Current Problem

**File:** `src/interaction.rs` (lines 235-247 and 276-292)

When lazy loading, we recalculate everything from scratch:

```rust
// Load 100 new candles (prepend or append)
chart.candles.extend(new_candles);

// Recalculate ALL values (including 10,000+ existing)
for (i, (name, period)) in indicator_info.iter().enumerate() {
    chart.indicators[i].values = MovingAverage::calculate_sma(&chart.candles, *period);
    // ☝️ O(n × 270) for entire dataset!
}
```

**Performance Impact:**
- 100 new candles loaded
- 10,000 existing candles already have MA values
- Recalculates all 10,100 values = wasted computation on 10,000 values

### Solution Strategy

Two cases to handle:

**Case 1: Prepend (Scrolling left into history)**
```
OLD: [candle_1000, candle_1001, ..., candle_10000]
NEW: [candle_900, ..., candle_999, candle_1000, ..., candle_10000]
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ 100 new candles
```
- Insert new candles at beginning
- Calculate MA only for new range [0..100]
- Keep existing MA values [100..10100]

**Case 2: Append (Scrolling right into recent data)**
```
OLD: [candle_1000, candle_1001, ..., candle_10000]
NEW: [candle_1000, candle_1001, ..., candle_10000, candle_10001, ..., candle_10100]
                                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                                                    100 new candles
```
- Add new candles at end
- Calculate MA only for new range [10000..10100]
- Keep existing MA values [0..10000]

**Expected Performance:**
- Before: 10,100 candles × 270 = 2.7M operations
- After: 100 candles × 270 = 27K operations
- **~100× faster lazy loading**

---

## Implementation Steps

### Step 1: Add Incremental Calculation Methods

**File:** `src/types.rs`

Add two new methods to `MovingAverage` impl:

```rust
impl MovingAverage {
    // ... existing calculate_sma() ...

    /// Calculate SMA for NEW candles being appended (scrolling right)
    /// Only calculates values starting from `old_len` index
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

    /// Calculate SMA for NEW candles being prepended (scrolling left into history)
    /// Calculates values for first `new_count` candles, then prepends to existing values
    pub fn calculate_sma_prepend(&mut self, candles: &[Candle], new_count: usize) {
        let period = self.period;
        let mut new_values = Vec::with_capacity(new_count);

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

        // Prepend new values to existing values
        new_values.append(&mut self.values);
        self.values = new_values;
    }

    /// Recalculate a specific range of MA values (for boundary corrections)
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
}
```

### Step 2: Update Lazy Load - Prepend Case

**File:** `src/interaction.rs` (around lines 217-249)

Replace the historical data loading section:

```rust
// Load more historical data when scrolling left
if start_idx < 20 && chart.candles.first().is_some() {
    chart.loading = true;

    let load_count = 100;
    let load_end_time = chart.candles.first().unwrap().time;

    // Calculate start time based on timeframe
    let interval_ms = match chart.timeframe.as_str() {
        "15m" => 15 * 60 * 1000,
        "1h" => 60 * 60 * 1000,
        "4h" => 4 * 60 * 60 * 1000,
        "1d" => 24 * 60 * 60 * 1000,
        _ => 60 * 60 * 1000,
    };
    let load_start_time = load_end_time - (load_count as i64 * interval_ms);

    if let Ok(new_candles) = db.load_candles(
        chart.ticker_id,
        &chart.timeframe,
        load_start_time,
        load_end_time - 1,
    ) {
        if !new_candles.is_empty() {
            println!("Lazy loaded {} historical candles", new_candles.len());

            // Store count before modification
            let new_len = new_candles.len();

            // Prepend new candles
            let mut combined = new_candles;
            combined.append(&mut chart.candles);
            chart.candles = combined;

            // Adjust visible_start to maintain view
            chart.visible_candle_start += new_len;

            // INCREMENTAL: Only calculate MA for NEW candles
            println!("Recalculating indicators for {} new candles (prepend)", new_len);
            for indicator in chart.indicators.iter_mut() {
                if indicator.name.starts_with("SMA") {
                    indicator.calculate_sma_prepend(&chart.candles, new_len);
                } else if indicator.name.starts_with("EMA") {
                    // EMA requires recursive calculation, must recalculate all
                    println!("Warning: EMA requires full recalculation");
                    indicator.values = MovingAverage::calculate_ema(&chart.candles, indicator.period);
                }
            }

            chart.needs_redraw = true;
        }
    }

    chart.loading = false;
}
```

### Step 3: Update Lazy Load - Append Case

**File:** `src/interaction.rs` (around lines 251-299)

Replace the recent data loading section:

```rust
// Load more recent data when scrolling right
if end_idx > chart.candles.len().saturating_sub(20) && chart.candles.last().is_some() {
    chart.loading = true;

    let load_start_time = chart.candles.last().unwrap().time;
    let interval_ms = match chart.timeframe.as_str() {
        "15m" => 15 * 60 * 1000,
        "1h" => 60 * 60 * 1000,
        "4h" => 4 * 60 * 60 * 1000,
        "1d" => 24 * 60 * 60 * 1000,
        _ => 60 * 60 * 1000,
    };
    let load_end_time = load_start_time + (100 * interval_ms);

    if let Ok(new_candles) = db.load_candles(
        chart.ticker_id,
        &chart.timeframe,
        load_start_time + 1,
        load_end_time,
    ) {
        if !new_candles.is_empty() {
            println!("Lazy loaded {} recent candles", new_candles.len());

            // Store old length before extending
            let old_len = chart.candles.len();

            // Append new candles
            chart.candles.extend(new_candles);

            // INCREMENTAL: Only calculate MA for NEW candles
            println!("Recalculating indicators from index {} (append)", old_len);
            for indicator in chart.indicators.iter_mut() {
                if indicator.name.starts_with("SMA") {
                    indicator.calculate_sma_append(&chart.candles, old_len);
                } else if indicator.name.starts_with("EMA") {
                    // EMA requires recursive calculation, must recalculate all
                    println!("Warning: EMA requires full recalculation");
                    indicator.values = MovingAverage::calculate_ema(&chart.candles, indicator.period);
                }
            }

            chart.needs_redraw = true;
        }
    }

    chart.loading = false;
}
```

### Step 4: Add Unit Tests

**File:** `src/types.rs` (add to test module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ... existing tests ...

    #[test]
    fn test_sma_append_incremental() {
        // Create initial dataset
        let mut candles = create_test_candles(1000);

        // Calculate full SMA
        let mut ma_incremental = MovingAverage::new_sma(&candles, 20, Color::WHITE);
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
        let mut ma_incremental = MovingAverage::new_sma(&candles_old, 20, Color::WHITE);
        let old_values = ma_incremental.values.clone();

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
        let mut ma = MovingAverage::new_sma(&candles, 20, Color::WHITE);

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
        let mut ma = MovingAverage::new_sma(&candles_old, 50, Color::WHITE);

        // Prepend only 10 candles (not enough for period=50)
        let mut candles_full = create_test_candles(10);
        candles_full.extend(candles_old);

        ma.calculate_sma_prepend(&candles_full, 10);

        // First 10 values should be None
        for i in 0..10 {
            assert!(ma.values[i].is_none(), "Index {} should be None", i);
        }
    }
}
```

### Step 5: Add Performance Logging (Temporary)

**File:** `src/interaction.rs`

Add timing logs to measure improvement:

```rust
use std::time::Instant;

// In prepend case:
let recalc_start = Instant::now();
for indicator in chart.indicators.iter_mut() {
    // ... recalculation code ...
}
let recalc_duration = recalc_start.elapsed();
println!("Indicator recalculation took: {:?}", recalc_duration);

// In append case (same pattern)
```

### Step 6: Run Tests

```bash
cargo test sma_append
cargo test sma_prepend
```

Expected: All tests pass

### Step 7: Integration Testing

Build and run:
```bash
cargo build --release
cargo run --release
```

**Test Scenario 1: Prepend (Historical)**
1. Load chart (shows last 50 candles)
2. Pan left slowly until lazy load triggers
3. Watch console for timing: `Indicator recalculation took: <5ms`
4. Verify MA lines extend smoothly into new data
5. No visual discontinuity at boundary

**Test Scenario 2: Append (Recent)**
1. Pan right to end of data
2. Continue panning right to trigger lazy load
3. Watch console for timing
4. Verify MA lines extend smoothly

**Test Scenario 3: Multiple Lazy Loads**
1. Pan left to trigger 5 lazy loads in a row
2. Each should be fast (<10ms)
3. Total dataset now has 500 new candles
4. Verify all MA values are correct

**Test Scenario 4: Edge Cases**
1. Load chart with <200 candles initially
2. SMA-200 should be empty
3. Pan left to load more candles
4. SMA-200 should appear once 200 candles loaded

### Step 8: Boundary Verification

The tricky part is the boundary between old and new data. Add extra verification:

**File:** `src/interaction.rs` (temporary debug code)

```rust
#[cfg(debug_assertions)]
{
    // Verify boundary values after incremental calculation
    for (idx, indicator) in chart.indicators.iter().enumerate() {
        // Recalculate boundary values from scratch
        let boundary_start = old_len.saturating_sub(indicator.period * 2);
        let boundary_end = (old_len + indicator.period * 2).min(chart.candles.len());

        for i in boundary_start..boundary_end {
            if i >= indicator.period - 1 {
                let expected_sum: f64 = chart.candles[i.saturating_sub(indicator.period - 1)..=i]
                    .iter()
                    .map(|c| c.close)
                    .sum();
                let expected = (expected_sum / indicator.period as f64) as f32;

                if let Some(actual) = indicator.values[i] {
                    let diff = (actual - expected).abs();
                    if diff > 0.01 {
                        eprintln!("WARNING: Indicator {} value mismatch at boundary index {}: {} vs {}, diff: {}",
                            indicator.name, i, actual, expected, diff);
                    }
                }
            }
        }
    }
}
```

Run in debug mode and watch for warnings:
```bash
cargo run
# Pan around and trigger lazy loads
# Should see no warnings
```

### Step 9: Handle EMA Edge Case

EMA requires special handling because it's recursive:

**Option 1: Accept full recalculation for EMA (current approach)**
- Simpler implementation
- EMA already O(n), not as slow as naive SMA
- Document limitation

**Option 2: Implement incremental EMA**
```rust
impl MovingAverage {
    pub fn calculate_ema_append(&mut self, candles: &[Candle], old_len: usize) {
        let period = self.period;
        let multiplier = 2.0 / (period as f64 + 1.0);

        for i in old_len..candles.len() {
            if i < period - 1 {
                self.values.push(None);
                continue;
            }

            if i == period - 1 && self.values.is_empty() {
                // First EMA value is SMA
                let sma: f64 = candles[0..period]
                    .iter()
                    .map(|c| c.close)
                    .sum::<f64>() / period as f64;
                self.values.push(Some(sma as f32));
            } else if i > 0 && self.values.len() == i {
                // Recursive: EMA[i] = price[i] * k + EMA[i-1] * (1-k)
                let prev = self.values[i - 1].unwrap();
                let ema = (candles[i].close as f32 - prev) * multiplier as f32 + prev;
                self.values.push(Some(ema));
            }
        }
    }
}
```

Choose Option 1 for now, implement Option 2 if EMA performance becomes an issue.

### Step 10: Remove Timing Logs

Once verified, remove temporary performance logging:

```bash
git diff src/interaction.rs  # Review changes
# Remove any Instant::now() and println! statements
```

### Step 11: Commit Changes

```bash
git add src/types.rs src/interaction.rs
git commit -m "Implement incremental MA recalculation on lazy load

Only recalculate MA values for newly loaded candles (append/prepend)
instead of recalculating entire dataset. Reduces lazy load lag by ~100×.

Changes:
- Add calculate_sma_append() for rightward scrolling
- Add calculate_sma_prepend() for leftward scrolling
- Add recalculate_range() for boundary corrections
- Update lazy load to use incremental methods
- Add comprehensive tests for boundary conditions

Performance (10,000 existing + 100 new candles):
- Before: 10,100 candles × 270 = 2.7M operations (~200ms)
- After: 100 candles × 270 = 27K operations (~2ms)
- 100× faster lazy loading

Known limitation: EMA still requires full recalculation (recursive dependency)

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Known Issues and Mitigations

### Issue 1: Floating Point Accumulation

**Problem:** With sliding window, small rounding errors might accumulate over many incremental updates.

**Mitigation:**
- Use f64 internally for sum calculation
- Periodically verify boundary values in debug builds
- Consider full recalculation every N lazy loads as safety check

**Code:**
```rust
// Every 10 lazy loads, recalculate from scratch
if chart.lazy_load_count % 10 == 0 {
    for indicator in chart.indicators.iter_mut() {
        indicator.values = MovingAverage::calculate_sma(&chart.candles, indicator.period);
    }
}
```

### Issue 2: Memory Reallocation

**Problem:** Prepending to Vec requires moving all elements.

**Mitigation:**
- Vec::append() after creating new vec is already optimal
- Consider using VecDeque if prepending becomes bottleneck
- Current approach is acceptable for 100-candle chunks

### Issue 3: Boundary Values at Prepend/Append Junction

**Problem:** First few values after new data might have precision issues.

**Mitigation:**
- `recalculate_range()` method can fix boundary if needed
- Add boundary verification in debug builds (Step 8)
- Extensive unit tests cover boundary cases

---

## Rollback Strategy

If issues discovered:

1. **Immediate:** Revert to full recalculation
   ```rust
   // In interaction.rs, replace incremental with:
   for indicator in chart.indicators.iter_mut() {
       if indicator.name.starts_with("SMA") {
           indicator.values = MovingAverage::calculate_sma(&chart.candles, indicator.period);
       }
   }
   ```

2. **Selective:** Keep Phase 1 (sliding window), revert Phase 2
   ```bash
   git revert HEAD  # Keep Phase 1 commit
   ```

3. **Hybrid:** Use incremental for append, full recalc for prepend
   ```rust
   // Prepend is less common, can use slower full recalc if issues
   ```

---

## Success Criteria

- [ ] All unit tests pass
- [ ] Prepend lazy load <10ms (was ~200ms)
- [ ] Append lazy load <10ms (was ~200ms)
- [ ] Visual continuity at boundaries
- [ ] No value mismatches in debug verification
- [ ] No memory leaks
- [ ] Multiple consecutive lazy loads work correctly
- [ ] Edge cases handled (insufficient period, single candle, etc.)

---

## Next Steps

After Phase 2 is complete:
- **Proceed to Phase 3:** Persistent Indicator Entities
- **Monitor:** Watch for any visual glitches over several days
- **Optimize:** Consider EMA incremental if needed
- **Document:** Update architecture docs with incremental calculation approach
