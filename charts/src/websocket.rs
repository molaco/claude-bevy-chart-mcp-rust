//! WebSocket streaming module for live Binance kline data, adapted for Bevy.
//!
//! This module provides real-time kline (candlestick) data from Binance via WebSocket
//! connection. It uses Bevy's async task system for background processing and
//! events for communicating updates to the main application.

use crate::types::Candle;

use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

// ============================================================================
// WEBSOCKET EVENTS
// ============================================================================

/// WebSocket events emitted to the Bevy event system.
#[derive(Message, Debug, Clone)]
pub enum WebSocketEvent {
    /// Successfully connected to the WebSocket stream
    Connected,
    /// Received a kline update
    Kline(Candle),
    /// Disconnected from the WebSocket stream
    Disconnected,
    /// An error occurred
    Error(String),
}

// ============================================================================
// BINANCE JSON STRUCTURES
// ============================================================================

/// Binance WebSocket kline event structure.
#[derive(Debug, Deserialize)]
pub struct BinanceKlineEvent {
    /// Event type (should be "kline")
    #[serde(rename = "e")]
    pub event_type: String,
    /// Event time
    #[serde(rename = "E")]
    #[allow(dead_code)]
    pub event_time: u64,
    /// Symbol
    #[serde(rename = "s")]
    #[allow(dead_code)]
    pub symbol: String,
    /// Kline data
    #[serde(rename = "k")]
    pub kline: BinanceKlineData,
}

/// Binance kline data within the WebSocket event.
#[derive(Debug, Deserialize)]
pub struct BinanceKlineData {
    /// Kline start time in milliseconds
    #[serde(rename = "t")]
    pub time: u64,
    /// Kline close time
    #[serde(rename = "T")]
    #[allow(dead_code)]
    pub close_time: u64,
    /// Symbol
    #[serde(rename = "s")]
    #[allow(dead_code)]
    pub symbol: String,
    /// Interval
    #[serde(rename = "i")]
    #[allow(dead_code)]
    pub interval: String,
    /// First trade ID
    #[serde(rename = "f")]
    #[allow(dead_code)]
    pub first_trade_id: i64,
    /// Last trade ID
    #[serde(rename = "L")]
    #[allow(dead_code)]
    pub last_trade_id: i64,
    /// Open price
    #[serde(rename = "o")]
    pub open: String,
    /// High price
    #[serde(rename = "h")]
    pub high: String,
    /// Low price
    #[serde(rename = "l")]
    pub low: String,
    /// Close price
    #[serde(rename = "c")]
    pub close: String,
    /// Volume
    #[serde(rename = "v")]
    pub volume: String,
    /// Number of trades
    #[serde(rename = "n")]
    #[allow(dead_code)]
    pub num_trades: u64,
    /// Is this kline closed?
    #[serde(rename = "x")]
    pub is_closed: bool,
    /// Quote asset volume
    #[serde(rename = "q")]
    #[allow(dead_code)]
    pub quote_volume: String,
    /// Taker buy base volume
    #[serde(rename = "V")]
    #[allow(dead_code)]
    pub taker_buy_base_volume: String,
    /// Taker buy quote volume
    #[serde(rename = "Q")]
    #[allow(dead_code)]
    pub taker_buy_quote_volume: String,
}

impl BinanceKlineData {
    /// Convert Binance kline data to our Candle struct.
    pub fn to_candle(&self) -> Result<Candle, String> {
        Ok(Candle {
            time: self.time as i64,
            open: self
                .open
                .parse()
                .map_err(|e| format!("Failed to parse open: {}", e))?,
            high: self
                .high
                .parse()
                .map_err(|e| format!("Failed to parse high: {}", e))?,
            low: self
                .low
                .parse()
                .map_err(|e| format!("Failed to parse low: {}", e))?,
            close: self
                .close
                .parse()
                .map_err(|e| format!("Failed to parse close: {}", e))?,
            volume: self
                .volume
                .parse()
                .map_err(|e| format!("Failed to parse volume: {}", e))?,
        })
    }
}

// ============================================================================
// BEVY RESOURCES
// ============================================================================

/// Configuration resource for WebSocket connection.
#[derive(Resource, Clone)]
pub struct WebSocketConfig {
    /// Trading pair symbol (e.g., "btcusdt")
    pub ticker: String,
    /// Kline interval (e.g., "1m", "5m", "1h")
    pub interval: String,
    /// Delay before attempting reconnection after disconnect
    pub reconnect_delay: Duration,
    /// Whether the WebSocket connection is enabled
    pub enabled: bool,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            ticker: "btcusdt".to_string(),
            interval: "1m".to_string(),
            reconnect_delay: Duration::from_secs(5),
            enabled: true,
        }
    }
}

impl WebSocketConfig {
    /// Create a new WebSocket configuration.
    pub fn new(ticker: impl Into<String>, interval: impl Into<String>) -> Self {
        Self {
            ticker: ticker.into(),
            interval: interval.into(),
            ..Default::default()
        }
    }

    /// Build the WebSocket URL for Binance kline stream.
    pub fn build_url(&self) -> String {
        let ticker_lower = self.ticker.to_lowercase();
        format!(
            "wss://stream.binance.com:9443/ws/{}@kline_{}",
            ticker_lower, self.interval
        )
    }
}

/// State resource for tracking WebSocket connection status.
#[derive(Resource, Default)]
pub struct WebSocketState {
    /// Whether currently connected to the WebSocket
    pub connected: bool,
    /// Time of the last received message
    pub last_message_time: Option<Instant>,
    /// Count of consecutive errors
    pub error_count: u32,
    /// Whether a WebSocket task is currently running
    pub task_running: bool,
}

impl WebSocketState {
    /// Update state when connected.
    pub fn on_connected(&mut self) {
        self.connected = true;
        self.last_message_time = Some(Instant::now());
        self.error_count = 0;
    }

    /// Update state when disconnected.
    pub fn on_disconnected(&mut self) {
        self.connected = false;
        self.task_running = false;
    }

    /// Update state when message received.
    pub fn on_message(&mut self) {
        self.last_message_time = Some(Instant::now());
    }

    /// Update state when error occurs.
    pub fn on_error(&mut self) {
        self.error_count += 1;
    }

    /// Check if connection appears stale (no messages for a while).
    pub fn is_stale(&self, timeout: Duration) -> bool {
        self.last_message_time
            .map(|t| t.elapsed() > timeout)
            .unwrap_or(false)
    }
}

/// Resource to hold the channel receiver for WebSocket events.
#[derive(Resource)]
pub struct WebSocketReceiver {
    pub receiver: Mutex<Receiver<WebSocketEvent>>,
}

/// Resource to hold the channel sender (for spawning new tasks).
#[derive(Resource, Clone)]
pub struct WebSocketSender {
    pub sender: Sender<WebSocketEvent>,
}

// ============================================================================
// ASYNC WEBSOCKET STREAM
// ============================================================================

/// State machine for the WebSocket connection.
enum ConnectionState {
    /// Initial state, ready to connect
    Disconnected,
    /// Connected and receiving messages
    Connected(
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ),
}

/// Async function that manages the WebSocket connection and streams events.
///
/// This function runs in a background task and sends events through the provided channel.
/// It handles:
/// - Connection establishment
/// - Ping-pong keep-alive
/// - Message parsing
/// - Automatic reconnection on disconnect
///
/// # Arguments
/// * `ticker` - The trading pair symbol (e.g., "btcusdt")
/// * `interval` - The kline interval (e.g., "1m", "5m", "1h")
/// * `sender` - Channel sender for WebSocket events
/// * `reconnect_delay` - Duration to wait before reconnecting
pub async fn websocket_stream(
    ticker: String,
    interval: String,
    sender: Sender<WebSocketEvent>,
    reconnect_delay: Duration,
) {
    let mut state = ConnectionState::Disconnected;

    loop {
        match state {
            ConnectionState::Disconnected => {
                // Build the WebSocket URL with lowercase ticker
                let ticker_lower = ticker.to_lowercase();
                let url = format!(
                    "wss://stream.binance.com:9443/ws/{}@kline_{}",
                    ticker_lower, interval
                );

                match connect_async(&url).await {
                    Ok((ws_stream, _)) => {
                        if sender.send(WebSocketEvent::Connected).is_err() {
                            // Receiver dropped, exit the task
                            return;
                        }
                        state = ConnectionState::Connected(ws_stream);
                    }
                    Err(e) => {
                        let _ = sender.send(WebSocketEvent::Error(format!(
                            "Connection failed: {}",
                            e
                        )));
                        // Wait before retrying
                        tokio::time::sleep(reconnect_delay).await;
                    }
                }
            }
            ConnectionState::Connected(mut ws_stream) => {
                match ws_stream.next().await {
                    Some(Ok(WsMessage::Text(text))) => {
                        // Parse the kline event
                        match serde_json::from_str::<BinanceKlineEvent>(&text) {
                            Ok(event) => {
                                if event.event_type == "kline" {
                                    match event.kline.to_candle() {
                                        Ok(candle) => {
                                            if sender.send(WebSocketEvent::Kline(candle)).is_err() {
                                                // Receiver dropped, exit the task
                                                return;
                                            }
                                        }
                                        Err(e) => {
                                            let _ = sender.send(WebSocketEvent::Error(e));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                // Log parse error but continue
                                let _ = sender.send(WebSocketEvent::Error(format!(
                                    "Parse error: {}",
                                    e
                                )));
                            }
                        }
                        state = ConnectionState::Connected(ws_stream);
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        // Respond to ping with pong for keep-alive
                        if let Err(e) = ws_stream.send(WsMessage::Pong(data)).await {
                            let _ = sender.send(WebSocketEvent::Error(format!(
                                "Failed to send pong: {}",
                                e
                            )));
                            let _ = sender.send(WebSocketEvent::Disconnected);
                            state = ConnectionState::Disconnected;
                            tokio::time::sleep(reconnect_delay).await;
                        } else {
                            state = ConnectionState::Connected(ws_stream);
                        }
                    }
                    Some(Ok(WsMessage::Pong(_))) => {
                        // Pong received, continue
                        state = ConnectionState::Connected(ws_stream);
                    }
                    Some(Ok(WsMessage::Close(_))) => {
                        let _ = sender.send(WebSocketEvent::Disconnected);
                        state = ConnectionState::Disconnected;
                        tokio::time::sleep(reconnect_delay).await;
                    }
                    Some(Ok(WsMessage::Binary(_))) | Some(Ok(WsMessage::Frame(_))) => {
                        // Skip binary/frame messages
                        state = ConnectionState::Connected(ws_stream);
                    }
                    Some(Err(e)) => {
                        let _ = sender.send(WebSocketEvent::Error(format!(
                            "WebSocket error: {}",
                            e
                        )));
                        let _ = sender.send(WebSocketEvent::Disconnected);
                        state = ConnectionState::Disconnected;
                        tokio::time::sleep(reconnect_delay).await;
                    }
                    None => {
                        // Stream ended
                        let _ = sender.send(WebSocketEvent::Disconnected);
                        state = ConnectionState::Disconnected;
                        tokio::time::sleep(reconnect_delay).await;
                    }
                }
            }
        }
    }
}

// ============================================================================
// BEVY SYSTEMS
// ============================================================================

/// System to spawn the WebSocket task when enabled.
///
/// This system checks if a WebSocket task should be started and spawns it
/// using Bevy's IoTaskPool for background async execution.
pub fn spawn_websocket_task(
    config: Res<WebSocketConfig>,
    mut state: ResMut<WebSocketState>,
    sender: Option<Res<WebSocketSender>>,
    mut commands: Commands,
) {
    // Don't spawn if disabled or already running
    if !config.enabled || state.task_running {
        return;
    }

    // Create channel if sender doesn't exist
    let sender = if let Some(s) = sender {
        s.sender.clone()
    } else {
        let (tx, rx) = channel();
        commands.insert_resource(WebSocketReceiver { receiver: Mutex::new(rx) });
        commands.insert_resource(WebSocketSender { sender: tx.clone() });
        tx
    };

    // Mark task as running
    state.task_running = true;

    // Clone config values for the async task
    let ticker = config.ticker.clone();
    let interval = config.interval.clone();
    let reconnect_delay = config.reconnect_delay;

    // Spawn the WebSocket task on the IoTaskPool
    IoTaskPool::get()
        .spawn(async move {
            websocket_stream(ticker, interval, sender, reconnect_delay).await;
        })
        .detach();
}

/// System to process incoming WebSocket events and forward them to Bevy's event system.
///
/// This system reads from the channel receiver and emits Bevy events for each
/// WebSocket event received. It also updates the WebSocketState resource.
pub fn process_websocket_events(
    receiver: Option<Res<WebSocketReceiver>>,
    mut state: ResMut<WebSocketState>,
    mut event_writer: MessageWriter<WebSocketEvent>,
) {
    let Some(receiver) = receiver else {
        return;
    };

    // Lock the receiver mutex
    let Ok(rx) = receiver.receiver.lock() else {
        return;
    };

    // Process all pending events from the channel
    while let Ok(event) = rx.try_recv() {
        // Update state based on event type
        match &event {
            WebSocketEvent::Connected => {
                state.on_connected();
            }
            WebSocketEvent::Kline(_) => {
                state.on_message();
            }
            WebSocketEvent::Disconnected => {
                state.on_disconnected();
            }
            WebSocketEvent::Error(_) => {
                state.on_error();
            }
        }

        // Forward event to Bevy's event system
        event_writer.write(event);
    }
}

/// System to handle reconnection when connection becomes stale.
///
/// This system monitors the connection state and triggers reconnection
/// if no messages have been received for a while.
pub fn check_connection_health(
    config: Res<WebSocketConfig>,
    mut state: ResMut<WebSocketState>,
    mut event_writer: MessageWriter<WebSocketEvent>,
) {
    if !config.enabled || !state.connected {
        return;
    }

    // Check for stale connection (no messages for 30 seconds)
    let stale_timeout = Duration::from_secs(30);
    if state.is_stale(stale_timeout) {
        event_writer.write(WebSocketEvent::Error(
            "Connection stale, no messages received".to_string(),
        ));
        state.on_disconnected();
    }
}

// ============================================================================
// PLUGIN
// ============================================================================

/// Plugin to add WebSocket functionality to a Bevy app.
///
/// # Example
/// ```ignore
/// use bevy::prelude::*;
/// use charts::websocket::{WebSocketPlugin, WebSocketConfig, WebSocketEvent};
///
/// fn main() {
///     App::new()
///         .add_plugins(DefaultPlugins)
///         .add_plugins(WebSocketPlugin)
///         .insert_resource(WebSocketConfig::new("btcusdt", "1m"))
///         .add_systems(Update, handle_kline_updates)
///         .run();
/// }
///
/// fn handle_kline_updates(mut events: EventReader<WebSocketEvent>) {
///     for event in events.read() {
///         match event {
///             WebSocketEvent::Kline(candle) => {
///                 println!("Received kline: {:?}", candle);
///             }
///             WebSocketEvent::Connected => println!("WebSocket connected"),
///             WebSocketEvent::Disconnected => println!("WebSocket disconnected"),
///             WebSocketEvent::Error(e) => eprintln!("WebSocket error: {}", e),
///         }
///     }
/// }
/// ```
pub struct WebSocketPlugin;

impl Plugin for WebSocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<WebSocketEvent>()
            .init_resource::<WebSocketConfig>()
            .init_resource::<WebSocketState>()
            .add_systems(
                Update,
                (
                    spawn_websocket_task,
                    process_websocket_events,
                    check_connection_health,
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

    #[test]
    fn test_binance_kline_parsing() {
        let json = r#"{
            "e": "kline",
            "E": 1638747660001,
            "s": "BTCUSDT",
            "k": {
                "t": 1638747660000,
                "T": 1638747719999,
                "s": "BTCUSDT",
                "i": "1m",
                "f": 100,
                "L": 200,
                "o": "48000.00",
                "c": "48050.00",
                "h": "48100.00",
                "l": "47900.00",
                "v": "100.5",
                "n": 100,
                "x": false,
                "q": "4810000.00",
                "V": "50.0",
                "Q": "2405000.00",
                "B": "0"
            }
        }"#;

        let event: BinanceKlineEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "kline");
        assert_eq!(event.kline.time, 1638747660000);
        assert!(!event.kline.is_closed);

        let candle = event.kline.to_candle().unwrap();
        assert_eq!(candle.time, 1638747660000);
        assert!((candle.open - 48000.0).abs() < 0.01);
        assert!((candle.high - 48100.0).abs() < 0.01);
        assert!((candle.low - 47900.0).abs() < 0.01);
        assert!((candle.close - 48050.0).abs() < 0.01);
        assert!((candle.volume - 100.5).abs() < 0.01);
    }

    #[test]
    fn test_url_construction() {
        let config = WebSocketConfig::new("BTCUSDT", "1m");
        assert_eq!(
            config.build_url(),
            "wss://stream.binance.com:9443/ws/btcusdt@kline_1m"
        );

        let config = WebSocketConfig::new("ethusdt", "5m");
        assert_eq!(
            config.build_url(),
            "wss://stream.binance.com:9443/ws/ethusdt@kline_5m"
        );
    }

    #[test]
    fn test_websocket_config_default() {
        let config = WebSocketConfig::default();
        assert_eq!(config.ticker, "btcusdt");
        assert_eq!(config.interval, "1m");
        assert_eq!(config.reconnect_delay, Duration::from_secs(5));
        assert!(config.enabled);
    }

    #[test]
    fn test_websocket_state_transitions() {
        let mut state = WebSocketState::default();
        assert!(!state.connected);
        assert!(state.last_message_time.is_none());
        assert_eq!(state.error_count, 0);

        // Test connected state
        state.on_connected();
        assert!(state.connected);
        assert!(state.last_message_time.is_some());
        assert_eq!(state.error_count, 0);

        // Test message received
        state.on_message();
        assert!(state.last_message_time.is_some());

        // Test error
        state.on_error();
        assert_eq!(state.error_count, 1);
        state.on_error();
        assert_eq!(state.error_count, 2);

        // Test disconnected
        state.on_disconnected();
        assert!(!state.connected);
        assert!(!state.task_running);
    }

    #[test]
    fn test_stale_connection_check() {
        let mut state = WebSocketState::default();

        // No message time set, should not be stale
        assert!(!state.is_stale(Duration::from_secs(30)));

        // Set message time to now
        state.on_message();
        assert!(!state.is_stale(Duration::from_secs(30)));

        // Set message time to past (simulate time passing)
        state.last_message_time = Some(Instant::now() - Duration::from_secs(60));
        assert!(state.is_stale(Duration::from_secs(30)));
    }

    #[test]
    fn test_binance_kline_closed() {
        let json = r#"{
            "e": "kline",
            "E": 1638747660001,
            "s": "BTCUSDT",
            "k": {
                "t": 1638747660000,
                "T": 1638747719999,
                "s": "BTCUSDT",
                "i": "1m",
                "f": 100,
                "L": 200,
                "o": "48000.00",
                "c": "48050.00",
                "h": "48100.00",
                "l": "47900.00",
                "v": "100.5",
                "n": 100,
                "x": true,
                "q": "4810000.00",
                "V": "50.0",
                "Q": "2405000.00",
                "B": "0"
            }
        }"#;

        let event: BinanceKlineEvent = serde_json::from_str(json).unwrap();
        assert!(event.kline.is_closed);
    }

    #[test]
    fn test_parse_error_handling() {
        // Test with invalid open price
        let json = r#"{
            "e": "kline",
            "E": 1638747660001,
            "s": "BTCUSDT",
            "k": {
                "t": 1638747660000,
                "T": 1638747719999,
                "s": "BTCUSDT",
                "i": "1m",
                "f": 100,
                "L": 200,
                "o": "invalid",
                "c": "48050.00",
                "h": "48100.00",
                "l": "47900.00",
                "v": "100.5",
                "n": 100,
                "x": false,
                "q": "4810000.00",
                "V": "50.0",
                "Q": "2405000.00",
                "B": "0"
            }
        }"#;

        let event: BinanceKlineEvent = serde_json::from_str(json).unwrap();
        let result = event.kline.to_candle();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse open"));
    }
}
