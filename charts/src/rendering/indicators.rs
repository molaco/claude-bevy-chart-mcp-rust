use crate::types::*;
use bevy::prelude::*;

pub fn render_moving_averages(mut gizmos: Gizmos, chart: Res<Chart>) {
    if chart.load_status != ChartLoadStatus::Ready {
        return;
    }

    let price_pane = chart.panes.iter().find(|p| matches!(p.id, PaneId::Price));
    if price_pane.is_none() {
        return;
    }
    let price_pane = price_pane.unwrap();

    let time_start = chart.visible_time_start;
    let time_end = chart.visible_time_end;

    // Collect visible candles as vec for MA value lookup by index
    let visible_candles: Vec<_> = chart.candles.range(time_start..=time_end).collect();
    if visible_candles.len() < 2 {
        return;
    }

    // Build a map from timestamp to global index for MA value lookup
    let timestamp_to_idx: std::collections::HashMap<i64, usize> = chart.candles
        .keys()
        .enumerate()
        .map(|(idx, &ts)| (ts, idx))
        .collect();

    // Render each Moving Average using Gizmos for smooth continuous lines
    for ma in &chart.indicators {
        if !ma.visible || ma.values.is_empty() {
            continue;
        }

        // Draw continuous line connecting MA points
        for window in visible_candles.windows(2) {
            let (curr_ts, _) = window[0];
            let (next_ts, _) = window[1];

            // Look up global index for MA value access
            let curr_idx = timestamp_to_idx.get(curr_ts).copied();
            let next_idx = timestamp_to_idx.get(next_ts).copied();

            if let (Some(ci), Some(ni)) = (curr_idx, next_idx) {
                if let (Some(Some(curr_value)), Some(Some(next_value))) =
                    (ma.values.get(ci), ma.values.get(ni))
                {
                    let curr_pos = price_pane.space.to_world(*curr_ts, *curr_value, time_start, time_end);
                    let next_pos = price_pane.space.to_world(*next_ts, *next_value, time_start, time_end);
                    gizmos.line_2d(curr_pos, next_pos, ma.color);
                }
            }
        }
    }
}
