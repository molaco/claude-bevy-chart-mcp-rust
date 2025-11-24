use crate::types::*;

/// Format volume numbers with K/M/B suffixes
pub fn format_volume(value: f32) -> String {
    if value >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else {
        format!("{:.2}", value)
    }
}

/// Find the candle nearest to a given timestamp
pub fn find_nearest_candle(
    candles: &std::collections::BTreeMap<i64, Candle>,
    target_time: i64,
) -> Option<(&i64, &Candle)> {
    if candles.is_empty() {
        return None;
    }

    // Get candle at or before target
    let before = candles.range(..=target_time).next_back();

    // Get candle after target
    let after = candles.range(target_time..).next();

    match (before, after) {
        (Some((t1, c1)), Some((t2, c2))) => {
            // Return the closer one
            if (target_time - t1).abs() <= (t2 - target_time).abs() {
                Some((t1, c1))
            } else {
                Some((t2, c2))
            }
        }
        (Some(entry), None) => Some(entry),
        (None, Some(entry)) => Some(entry),
        (None, None) => None,
    }
}
