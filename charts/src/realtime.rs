//! Real-time data integration systems for Bevy.
//!
//! This module integrates WebSocket updates with the chart:
//! - Buffered updates with throttling to prevent excessive redraws
//! - Request deduplication to prevent duplicate API calls
//! - Viewport-aware memory management for efficient cleanup

use bevy::prelude::*;
use std::collections::HashSet;
use std::time::Instant;

use crate::types::Candle;

// Logging macros - these are no-ops since bevy_log feature is not enabled.
// Enable bevy_log feature in Cargo.toml to activate logging.
macro_rules! info { ($($arg:tt)*) => {} }
macro_rules! warn { ($($arg:tt)*) => {} }
macro_rules! error { ($($arg:tt)*) => {} }
macro_rules! debug { ($($arg:tt)*) => {} }
macro_rules! trace { ($($arg:tt)*) => {} }

// ============================================================================
// REALTIME CONFIGURATION
// ============================================================================

/// Configuration for real-time data updates.
///
/// Controls WebSocket connection parameters and update throttling.
#[derive(Resource, Clone, Debug)]
pub struct RealtimeConfig {
    /// Whether real-time updates are enabled
    pub enabled: bool,
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub ticker: String,
    /// Timeframe for kline updates (e.g., "1m", "15m", "1h")
    pub timeframe: String,
    /// Minimum milliseconds between applying buffered updates
    /// Default: 100ms to prevent excessive redraws while maintaining responsiveness
    pub throttle_ms: u64,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ticker: String::new(),
            timeframe: String::new(),
            throttle_ms: 100,
        }
    }
}

impl RealtimeConfig {
    /// Create a new RealtimeConfig with the specified parameters.
    pub fn new(ticker: impl Into<String>, timeframe: impl Into<String>) -> Self {
        Self {
            enabled: true,
            ticker: ticker.into(),
            timeframe: timeframe.into(),
            throttle_ms: 100,
        }
    }

    /// Set the throttle interval in milliseconds.
    pub fn with_throttle_ms(mut self, throttle_ms: u64) -> Self {
        self.throttle_ms = throttle_ms;
        self
    }

    /// Enable or disable real-time updates.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns the WebSocket URL for Binance kline stream.
    pub fn websocket_url(&self) -> String {
        format!(
            "wss://stream.binance.com:9443/ws/{}@kline_{}",
            self.ticker.to_lowercase(),
            self.timeframe
        )
    }
}

// ============================================================================
// REALTIME STATE
// ============================================================================

/// State for buffering and applying real-time updates.
///
/// Updates are buffered and applied at throttled intervals to prevent
/// excessive chart redraws while maintaining data freshness.
#[derive(Resource, Default)]
pub struct RealtimeState {
    /// Buffered candle updates waiting to be applied
    pub pending_updates: Vec<Candle>,
    /// Whether there are pending updates that need to be applied
    pub needs_apply: bool,
    /// Timestamp of the last applied update (for throttling)
    pub last_update: Option<Instant>,
    /// Connection status
    pub connection_status: ConnectionStatus,
    /// Number of updates received since last apply
    pub updates_since_apply: usize,
}

/// WebSocket connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

impl RealtimeState {
    /// Create a new RealtimeState.
    pub fn new() -> Self {
        Self::default()
    }

    /// Buffer a candle update for later application.
    pub fn buffer_update(&mut self, candle: Candle) {
        // Check if we already have an update for this timestamp
        if let Some(existing) = self
            .pending_updates
            .iter_mut()
            .find(|c| c.time == candle.time)
        {
            // Update existing candle (newer data replaces old)
            *existing = candle;
        } else {
            self.pending_updates.push(candle);
        }
        self.needs_apply = true;
        self.updates_since_apply += 1;
    }

    /// Check if enough time has passed since last update application.
    pub fn should_apply(&self, throttle_ms: u64) -> bool {
        if !self.needs_apply || self.pending_updates.is_empty() {
            return false;
        }

        match self.last_update {
            Some(last) => last.elapsed().as_millis() >= throttle_ms as u128,
            None => true, // First update, apply immediately
        }
    }

    /// Take all pending updates and reset state.
    /// Returns the buffered updates for application.
    pub fn take_pending_updates(&mut self) -> Vec<Candle> {
        self.needs_apply = false;
        self.last_update = Some(Instant::now());
        self.updates_since_apply = 0;
        std::mem::take(&mut self.pending_updates)
    }

    /// Clear all pending updates without applying.
    pub fn clear_pending(&mut self) {
        self.pending_updates.clear();
        self.needs_apply = false;
        self.updates_since_apply = 0;
    }

    /// Set the connection status.
    pub fn set_status(&mut self, status: ConnectionStatus) {
        self.connection_status = status;
    }
}

// ============================================================================
// PENDING FETCHES (Request Deduplication)
// ============================================================================

/// Resource for tracking pending API requests to prevent duplicates.
///
/// Uses (start_time, end_time) tuples to identify unique requests.
#[derive(Resource, Default)]
pub struct PendingFetches {
    /// Set of pending request ranges: (start_time, end_time)
    pub requests: HashSet<(i64, i64)>,
    /// Whether a fetch operation is currently in progress
    pub fetching: bool,
    /// Maximum concurrent requests allowed
    pub max_concurrent: usize,
}

impl PendingFetches {
    /// Create a new PendingFetches resource.
    pub fn new() -> Self {
        Self {
            requests: HashSet::new(),
            fetching: false,
            max_concurrent: 3,
        }
    }

    /// Set maximum concurrent requests.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Check if a request for the given range is already pending.
    pub fn is_pending(&self, start_time: i64, end_time: i64) -> bool {
        self.requests.contains(&(start_time, end_time))
    }

    /// Check if we can start a new fetch (not at max concurrent).
    pub fn can_fetch(&self) -> bool {
        self.requests.len() < self.max_concurrent
    }

    /// Register a new pending request.
    /// Returns true if the request was added, false if it already exists.
    pub fn add_request(&mut self, start_time: i64, end_time: i64) -> bool {
        if self.requests.contains(&(start_time, end_time)) {
            return false;
        }
        self.requests.insert((start_time, end_time));
        self.fetching = !self.requests.is_empty();
        true
    }

    /// Remove a completed request.
    pub fn remove_request(&mut self, start_time: i64, end_time: i64) {
        self.requests.remove(&(start_time, end_time));
        self.fetching = !self.requests.is_empty();
    }

    /// Clear all pending requests.
    pub fn clear(&mut self) {
        self.requests.clear();
        self.fetching = false;
    }

    /// Check if any fetch is in progress.
    pub fn is_fetching(&self) -> bool {
        self.fetching
    }

    /// Get the number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.requests.len()
    }
}

// ============================================================================
// MEMORY MANAGEMENT
// ============================================================================

/// Resource for viewport-aware memory cleanup.
///
/// Manages memory by removing old candles when the dataset grows too large,
/// but only when viewing live (rightmost) data to preserve historical exploration.
#[derive(Resource, Clone, Debug)]
pub struct MemoryManager {
    /// Maximum number of candles to keep in memory
    pub max_candles: usize,
    /// Threshold at which cleanup is triggered
    pub cleanup_threshold: usize,
    /// Number of candles to remove during cleanup
    pub cleanup_amount: usize,
    /// Whether cleanup is currently enabled
    pub enabled: bool,
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self {
            max_candles: 10000,
            cleanup_threshold: 9000,
            cleanup_amount: 1000,
            enabled: true,
        }
    }
}

impl MemoryManager {
    /// Create a new MemoryManager with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum candles to keep.
    pub fn with_max_candles(mut self, max: usize) -> Self {
        self.max_candles = max;
        self
    }

    /// Set cleanup threshold.
    pub fn with_cleanup_threshold(mut self, threshold: usize) -> Self {
        self.cleanup_threshold = threshold;
        self
    }

    /// Set cleanup amount.
    pub fn with_cleanup_amount(mut self, amount: usize) -> Self {
        self.cleanup_amount = amount;
        self
    }

    /// Enable or disable automatic cleanup.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Check if cleanup should be triggered based on current candle count.
    pub fn should_cleanup(&self, current_count: usize) -> bool {
        self.enabled && current_count >= self.cleanup_threshold
    }

    /// Calculate how many candles to remove.
    /// Ensures we don't remove more than available.
    pub fn candles_to_remove(&self, current_count: usize) -> usize {
        if current_count <= self.max_candles - self.cleanup_amount {
            0
        } else {
            self.cleanup_amount.min(current_count.saturating_sub(self.max_candles - self.cleanup_amount))
        }
    }
}

// ============================================================================
// VIEWPORT STATE (for cleanup decisions)
// ============================================================================

/// Tracks viewport position relative to data for cleanup decisions.
#[derive(Resource, Default)]
pub struct ViewportState {
    /// Whether the viewport is showing the rightmost (live) data
    pub viewing_live: bool,
    /// Index of the rightmost visible candle
    pub rightmost_visible_index: usize,
    /// Total number of candles available
    pub total_candles: usize,
    /// Threshold for considering viewport as "live" (candles from end)
    pub live_threshold: usize,
}

impl ViewportState {
    /// Create a new ViewportState.
    pub fn new() -> Self {
        Self {
            viewing_live: true,
            rightmost_visible_index: 0,
            total_candles: 0,
            live_threshold: 10, // Within 10 candles of the end = live
        }
    }

    /// Update the viewport state based on visible range.
    pub fn update(
        &mut self,
        visible_start: usize,
        visible_count: usize,
        total_candles: usize,
    ) {
        self.total_candles = total_candles;
        self.rightmost_visible_index = visible_start.saturating_add(visible_count);

        // Viewing live if the rightmost visible is within threshold of total
        self.viewing_live = total_candles.saturating_sub(self.rightmost_visible_index) <= self.live_threshold;
    }

    /// Check if we're viewing live data (rightmost region).
    pub fn is_viewing_live(&self) -> bool {
        self.viewing_live
    }

    /// Set the threshold for live detection.
    pub fn with_live_threshold(mut self, threshold: usize) -> Self {
        self.live_threshold = threshold;
        self
    }
}

// ============================================================================
// BEVY SYSTEMS
// ============================================================================

/// Startup system to initialize WebSocket connection resources.
///
/// This system sets up the necessary resources for real-time data handling.
/// The actual WebSocket connection should be managed by an async runtime
/// external to Bevy, sending events through channels.
pub fn setup_realtime_connection(mut commands: Commands) {
    // Initialize resources if not already present
    commands.init_resource::<RealtimeConfig>();
    commands.init_resource::<RealtimeState>();
    commands.init_resource::<PendingFetches>();
    commands.init_resource::<MemoryManager>();
    commands.init_resource::<ViewportState>();

    info!("Real-time connection resources initialized");
}

/// Update system to buffer incoming kline updates.
///
/// This system receives kline updates and buffers them without immediately
/// clearing the chart cache. Updates are applied later by `apply_throttled_updates`.
///
/// # Arguments
/// * `events` - Event reader for incoming KlineUpdate events
/// * `realtime_state` - State for buffering updates
/// * `config` - Real-time configuration
pub fn receive_kline_updates(
    mut events: MessageReader<KlineUpdateEvent>,
    mut realtime_state: ResMut<RealtimeState>,
    config: Res<RealtimeConfig>,
) {
    if !config.enabled {
        return;
    }

    for event in events.read() {
        match event {
            KlineUpdateEvent::Connected => {
                realtime_state.set_status(ConnectionStatus::Connected);
                info!("WebSocket connected for {} {}", config.ticker, config.timeframe);
            }
            KlineUpdateEvent::Kline(candle) => {
                // Buffer the update without immediate cache invalidation
                realtime_state.buffer_update(candle.clone());
                trace!(
                    "Buffered kline update: time={}, close={}",
                    candle.time,
                    candle.close
                );
            }
            KlineUpdateEvent::Disconnected => {
                realtime_state.set_status(ConnectionStatus::Disconnected);
                warn!("WebSocket disconnected");
            }
            KlineUpdateEvent::Error(msg) => {
                realtime_state.set_status(ConnectionStatus::Error);
                error!("WebSocket error: {}", msg);
            }
        }
    }
}

/// Update system that applies buffered updates at throttled intervals.
///
/// This system checks if enough time has passed since the last update
/// application, then applies all buffered candle updates to the chart.
///
/// # Arguments
/// * `realtime_state` - State containing buffered updates
/// * `config` - Real-time configuration with throttle settings
/// * `chart` - The chart resource to update (must be defined in types module)
pub fn apply_throttled_updates(
    mut realtime_state: ResMut<RealtimeState>,
    config: Res<RealtimeConfig>,
    // Note: Chart resource should be imported from types module
    // mut chart: ResMut<Chart>,
) {
    if !config.enabled {
        return;
    }

    // Check if we should apply updates based on throttle timing
    if !realtime_state.should_apply(config.throttle_ms) {
        return;
    }

    // Take all pending updates
    let updates = realtime_state.take_pending_updates();

    if updates.is_empty() {
        return;
    }

    debug!(
        "Applying {} buffered kline updates",
        updates.len()
    );

    // Apply updates to chart
    // This is a placeholder - actual implementation depends on Chart structure
    // for candle in updates {
    //     // Insert or update candle in chart data
    //     // chart.update_candle(candle);
    // }

    // Mark chart for redraw
    // chart.needs_redraw = true;

    info!("Applied {} real-time updates", updates.len());
}

/// Helper function to check fetch deduplication.
///
/// Returns true if a fetch request for the given range should proceed,
/// false if it's already pending.
pub fn check_fetch_deduplication(
    pending_fetches: &PendingFetches,
    start_time: i64,
    end_time: i64,
) -> bool {
    // Check if request is already pending
    if pending_fetches.is_pending(start_time, end_time) {
        trace!(
            "Skipping duplicate fetch request: {} - {}",
            start_time,
            end_time
        );
        return false;
    }

    // Check if we're at max concurrent requests
    if !pending_fetches.can_fetch() {
        trace!(
            "At max concurrent fetches ({}), deferring request",
            pending_fetches.pending_count()
        );
        return false;
    }

    true
}

/// System to clean up old candles based on viewport position.
///
/// Only removes old candles when:
/// 1. Memory threshold is exceeded
/// 2. User is viewing live (rightmost) data
///
/// This preserves historical data during exploration while managing memory
/// during live trading scenarios.
pub fn cleanup_old_candles(
    viewport_state: Res<ViewportState>,
    memory_manager: Res<MemoryManager>,
    // Note: Chart resource should be imported from types module
    // mut chart: ResMut<Chart>,
) {
    // Skip if memory management is disabled
    if !memory_manager.enabled {
        return;
    }

    // Only cleanup when viewing live data
    if !viewport_state.is_viewing_live() {
        trace!("Skipping cleanup: not viewing live data");
        return;
    }

    // Check if cleanup is needed based on candle count
    let current_count = viewport_state.total_candles;
    if !memory_manager.should_cleanup(current_count) {
        return;
    }

    let to_remove = memory_manager.candles_to_remove(current_count);
    if to_remove == 0 {
        return;
    }

    // Perform cleanup
    // This is a placeholder - actual implementation depends on Chart structure
    // if chart.candles.len() > to_remove {
    //     chart.candles.drain(0..to_remove);
    //     chart.candle_offset += to_remove;
    //     chart.visible_candle_start = chart.visible_candle_start.saturating_sub(to_remove);
    //     chart.needs_redraw = true;
    //
    //     // Also update indicators
    //     for indicator in &mut chart.indicators {
    //         if indicator.values.len() > to_remove {
    //             indicator.values.drain(0..to_remove);
    //         }
    //     }
    //
    //     info!("Cleaned up {} old candles", to_remove);
    // }

    debug!(
        "Would cleanup {} candles (current: {}, threshold: {})",
        to_remove, current_count, memory_manager.cleanup_threshold
    );
}

/// System to update viewport state based on chart position.
///
/// This should run after interaction handling to keep viewport state current.
pub fn update_viewport_state(
    mut viewport_state: ResMut<ViewportState>,
    // Note: Chart resource should be imported from types module
    // chart: Res<Chart>,
) {
    // Placeholder implementation
    // viewport_state.update(
    //     chart.visible_candle_start,
    //     chart.visible_candle_count,
    //     chart.candles.len(),
    // );
    let _ = &mut viewport_state; // Silence unused warning
}

// ============================================================================
// EVENTS
// ============================================================================

/// Event for WebSocket kline updates.
#[derive(Message, Debug, Clone)]
pub enum KlineUpdateEvent {
    /// Successfully connected to WebSocket
    Connected,
    /// Received a kline/candle update
    Kline(Candle),
    /// Disconnected from WebSocket
    Disconnected,
    /// Error occurred
    Error(String),
}

// ============================================================================
// PLUGIN
// ============================================================================

/// Plugin for real-time data integration.
///
/// Adds all necessary resources and systems for handling WebSocket updates,
/// buffered updates, request deduplication, and memory management.
pub struct RealtimePlugin;

impl Plugin for RealtimePlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources
            .init_resource::<RealtimeConfig>()
            .init_resource::<RealtimeState>()
            .init_resource::<PendingFetches>()
            .init_resource::<MemoryManager>()
            .init_resource::<ViewportState>()
            // Messages (events)
            .add_message::<KlineUpdateEvent>()
            // Systems
            .add_systems(Startup, setup_realtime_connection)
            .add_systems(
                Update,
                (
                    receive_kline_updates,
                    apply_throttled_updates,
                    update_viewport_state,
                    cleanup_old_candles,
                )
                    .chain(),
            );
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candle(time: i64, close: f64) -> Candle {
        Candle {
            time,
            open: close - 1.0,
            high: close + 1.0,
            low: close - 2.0,
            close,
            volume: 1000.0,
        }
    }

    #[test]
    fn test_realtime_config_default() {
        let config = RealtimeConfig::default();
        assert!(!config.enabled);
        assert!(config.ticker.is_empty());
        assert!(config.timeframe.is_empty());
        assert_eq!(config.throttle_ms, 100);
    }

    #[test]
    fn test_realtime_config_new() {
        let config = RealtimeConfig::new("BTCUSDT", "1m")
            .with_throttle_ms(50)
            .with_enabled(true);

        assert!(config.enabled);
        assert_eq!(config.ticker, "BTCUSDT");
        assert_eq!(config.timeframe, "1m");
        assert_eq!(config.throttle_ms, 50);
    }

    #[test]
    fn test_realtime_config_websocket_url() {
        let config = RealtimeConfig::new("BTCUSDT", "1m");
        assert_eq!(
            config.websocket_url(),
            "wss://stream.binance.com:9443/ws/btcusdt@kline_1m"
        );
    }

    #[test]
    fn test_realtime_state_buffer_update() {
        let mut state = RealtimeState::new();

        let candle1 = create_test_candle(1000, 50000.0);
        let candle2 = create_test_candle(2000, 51000.0);

        state.buffer_update(candle1);
        assert_eq!(state.pending_updates.len(), 1);
        assert!(state.needs_apply);
        assert_eq!(state.updates_since_apply, 1);

        state.buffer_update(candle2);
        assert_eq!(state.pending_updates.len(), 2);
        assert_eq!(state.updates_since_apply, 2);
    }

    #[test]
    fn test_realtime_state_buffer_update_same_timestamp() {
        let mut state = RealtimeState::new();

        let candle1 = create_test_candle(1000, 50000.0);
        let candle2 = create_test_candle(1000, 50500.0); // Same timestamp, different close

        state.buffer_update(candle1);
        state.buffer_update(candle2);

        assert_eq!(state.pending_updates.len(), 1);
        assert_eq!(state.pending_updates[0].close, 50500.0); // Should be updated
    }

    #[test]
    fn test_realtime_state_take_pending() {
        let mut state = RealtimeState::new();

        state.buffer_update(create_test_candle(1000, 50000.0));
        state.buffer_update(create_test_candle(2000, 51000.0));

        let updates = state.take_pending_updates();

        assert_eq!(updates.len(), 2);
        assert!(state.pending_updates.is_empty());
        assert!(!state.needs_apply);
        assert!(state.last_update.is_some());
        assert_eq!(state.updates_since_apply, 0);
    }

    #[test]
    fn test_realtime_state_should_apply() {
        let mut state = RealtimeState::new();

        // No pending updates
        assert!(!state.should_apply(100));

        // With pending update, no last_update
        state.buffer_update(create_test_candle(1000, 50000.0));
        assert!(state.should_apply(100)); // First update, apply immediately

        // After taking updates
        let _ = state.take_pending_updates();
        assert!(!state.should_apply(100)); // No pending updates
    }

    #[test]
    fn test_pending_fetches_deduplication() {
        let mut fetches = PendingFetches::new();

        assert!(fetches.add_request(1000, 2000));
        assert!(!fetches.add_request(1000, 2000)); // Duplicate
        assert!(fetches.add_request(2000, 3000)); // Different range

        assert!(fetches.is_pending(1000, 2000));
        assert!(!fetches.is_pending(3000, 4000));

        fetches.remove_request(1000, 2000);
        assert!(!fetches.is_pending(1000, 2000));
    }

    #[test]
    fn test_pending_fetches_max_concurrent() {
        let mut fetches = PendingFetches::new().with_max_concurrent(2);

        assert!(fetches.can_fetch());
        fetches.add_request(1000, 2000);
        assert!(fetches.can_fetch());
        fetches.add_request(2000, 3000);
        assert!(!fetches.can_fetch()); // At max

        fetches.remove_request(1000, 2000);
        assert!(fetches.can_fetch());
    }

    #[test]
    fn test_memory_manager_should_cleanup() {
        let manager = MemoryManager::default();

        assert!(!manager.should_cleanup(5000)); // Below threshold
        assert!(manager.should_cleanup(9000)); // At threshold
        assert!(manager.should_cleanup(10000)); // Above threshold

        let disabled = MemoryManager::default().with_enabled(false);
        assert!(!disabled.should_cleanup(10000)); // Disabled
    }

    #[test]
    fn test_memory_manager_candles_to_remove() {
        let manager = MemoryManager {
            max_candles: 10000,
            cleanup_threshold: 9000,
            cleanup_amount: 1000,
            enabled: true,
        };

        assert_eq!(manager.candles_to_remove(8000), 0); // Below max - cleanup_amount
        assert_eq!(manager.candles_to_remove(9500), 500); // Partial removal
        assert_eq!(manager.candles_to_remove(10000), 1000); // Full cleanup_amount
    }

    #[test]
    fn test_viewport_state_viewing_live() {
        let mut viewport = ViewportState::new();

        // Viewing end of data
        viewport.update(900, 100, 1000);
        assert!(viewport.is_viewing_live());

        // Viewing historical data
        viewport.update(0, 100, 1000);
        assert!(!viewport.is_viewing_live());

        // Within threshold of end
        viewport.update(885, 100, 1000);
        assert!(viewport.is_viewing_live()); // 985 + 10 >= 1000
    }

    #[test]
    fn test_check_fetch_deduplication() {
        let mut fetches = PendingFetches::new();

        assert!(check_fetch_deduplication(&fetches, 1000, 2000));

        fetches.add_request(1000, 2000);
        assert!(!check_fetch_deduplication(&fetches, 1000, 2000)); // Duplicate
        assert!(check_fetch_deduplication(&fetches, 2000, 3000)); // Different
    }

    #[test]
    fn test_connection_status_default() {
        let state = RealtimeState::new();
        assert_eq!(state.connection_status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_connection_status_transitions() {
        let mut state = RealtimeState::new();

        state.set_status(ConnectionStatus::Connecting);
        assert_eq!(state.connection_status, ConnectionStatus::Connecting);

        state.set_status(ConnectionStatus::Connected);
        assert_eq!(state.connection_status, ConnectionStatus::Connected);

        state.set_status(ConnectionStatus::Error);
        assert_eq!(state.connection_status, ConnectionStatus::Error);
    }
}
