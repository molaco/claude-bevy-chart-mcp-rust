use bevy::prelude::*;

use crate::config::InteractionConfig;
use crate::interaction::state::InteractionState;
use crate::types::{
    CandleData, ChartDatabase, ChartMetadata, IndicatorState, MovingAverage, ViewportState,
};

// ============================================================================
// LAZY LOAD SYSTEM
// ============================================================================

/// Check if more candle data should be loaded based on viewport position.
/// Coordinates with interaction state to prevent loading during active interactions.
pub fn check_lazy_load(
    mut candle_data: ResMut<CandleData>,
    mut viewport_state: ResMut<ViewportState>,
    mut indicator_state: ResMut<IndicatorState>,
    chart_metadata: Res<ChartMetadata>,
    interaction_config: Res<InteractionConfig>,
    interaction: Res<InteractionState>,
    db: Res<ChartDatabase>,
) {
    // Don't load during active interactions to prevent viewport jumps
    if interaction.mode.is_active() {
        return;
    }

    if viewport_state.loading {
        return; // Already loading
    }

    let start_idx = viewport_state.visible_candle_start;
    let end_idx = start_idx + viewport_state.visible_candle_count;

    // Load more historical data when scrolling left
    if start_idx < interaction_config.lazy_load_threshold
        && candle_data.candles.first().is_some()
    {
        load_historical_data(
            &mut candle_data,
            &mut viewport_state,
            &mut indicator_state,
            &chart_metadata,
            &interaction_config,
            &db,
        );
    }

    // Load more recent data when scrolling right
    if end_idx
        > candle_data
            .candles
            .len()
            .saturating_sub(interaction_config.lazy_load_threshold)
        && candle_data.candles.last().is_some()
    {
        load_recent_data(
            &mut candle_data,
            &mut viewport_state,
            &mut indicator_state,
            &chart_metadata,
            &interaction_config,
            &db,
        );
    }
}

/// Load historical candles (prepend to start of data).
fn load_historical_data(
    candle_data: &mut ResMut<CandleData>,
    viewport_state: &mut ResMut<ViewportState>,
    indicator_state: &mut ResMut<IndicatorState>,
    chart_metadata: &Res<ChartMetadata>,
    interaction_config: &Res<InteractionConfig>,
    db: &Res<ChartDatabase>,
) {
    viewport_state.loading = true;

    let load_count = interaction_config.lazy_load_batch_size;
    let load_end_time = candle_data.candles.first().unwrap().time;

    // Calculate start time based on timeframe
    let interval_ms = chart_metadata.interval_ms();
    let load_start_time = load_end_time - (load_count as i64 * interval_ms);

    if let Ok(new_candles) = db.load_candles(
        chart_metadata.ticker_id,
        chart_metadata.timeframe(),
        load_start_time,
        load_end_time - 1, // Exclude the first candle we already have
    ) {
        if !new_candles.is_empty() {
            // Prepend new candles
            let new_len = new_candles.len();
            let mut combined = new_candles;
            combined.append(&mut candle_data.candles);
            candle_data.candles = combined;

            // Adjust visible_start to maintain view
            viewport_state.visible_candle_start += new_len;

            // Update indicators incrementally
            for indicator in indicator_state.indicators.iter_mut() {
                if indicator.name.starts_with("SMA") {
                    indicator.calculate_sma_prepend(&candle_data.candles, new_len);
                } else if indicator.name.starts_with("EMA") {
                    // EMA requires recursive calculation, must recalculate all
                    indicator.values =
                        MovingAverage::calculate_ema(&candle_data.candles, indicator.period);
                }
            }

            viewport_state.needs_redraw = true;
        }
    }

    viewport_state.loading = false;
}

/// Load recent candles (append to end of data).
fn load_recent_data(
    candle_data: &mut ResMut<CandleData>,
    viewport_state: &mut ResMut<ViewportState>,
    indicator_state: &mut ResMut<IndicatorState>,
    chart_metadata: &Res<ChartMetadata>,
    interaction_config: &Res<InteractionConfig>,
    db: &Res<ChartDatabase>,
) {
    viewport_state.loading = true;

    let load_start_time = candle_data.candles.last().unwrap().time;
    let interval_ms = chart_metadata.interval_ms();
    let load_end_time = load_start_time + (interaction_config.lazy_load_batch_size * interval_ms);

    if let Ok(new_candles) = db.load_candles(
        chart_metadata.ticker_id,
        chart_metadata.timeframe(),
        load_start_time + 1, // Exclude the last candle we already have
        load_end_time,
    ) {
        if !new_candles.is_empty() {
            // Store old length before extending
            let old_len = candle_data.candles.len();

            // Append new candles
            candle_data.candles.extend(new_candles);

            // Update indicators incrementally
            for indicator in indicator_state.indicators.iter_mut() {
                if indicator.name.starts_with("SMA") {
                    indicator.calculate_sma_append(&candle_data.candles, old_len);
                } else if indicator.name.starts_with("EMA") {
                    // EMA requires recursive calculation, must recalculate all
                    indicator.values =
                        MovingAverage::calculate_ema(&candle_data.candles, indicator.period);
                }
            }

            viewport_state.needs_redraw = true;
        }
    }

    viewport_state.loading = false;
}
