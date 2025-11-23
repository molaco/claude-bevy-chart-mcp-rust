//! Multi-tier cache invalidation system for Bevy chart rendering.
//!
//! This module implements a selective cache invalidation strategy that separates
//! expensive rendering operations from cheap overlay updates. The goal is to
//! minimize unnecessary GPU work by only invalidating the caches that are
//! affected by a given change.
//!
//! # Caching Strategy
//!
//! The cache is divided into multiple tiers based on rendering cost:
//!
//! - **Main cache (expensive)**: Candlestick rendering with potentially thousands
//!   of primitives. Only invalidated on pan, zoom, resize, or data updates.
//!
//! - **Crosshair cache (cheap)**: Simple overlay with two lines and optional
//!   tooltip. Invalidated on every cursor movement.
//!
//! - **Label caches**: X-axis (time) and Y-axis (price) labels. Invalidated
//!   on view changes but can be updated independently.
//!
//! # Throttling
//!
//! For high-frequency updates (e.g., WebSocket price feeds), the [`ThrottleState`]
//! resource implements frame-rate throttling to prevent excessive redraws.
//! Updates are batched and rendered at a configurable interval (default 100ms).
//!
//! # Usage with Bevy
//!
//! Both [`RenderCache`] and [`ThrottleState`] derive `Resource` for easy
//! integration with Bevy's ECS:
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//! use charts::cache::{RenderCache, ThrottleState};
//!
//! fn setup(mut commands: Commands) {
//!     commands.insert_resource(RenderCache::new());
//!     commands.insert_resource(ThrottleState::default());
//! }
//!
//! fn handle_pan(mut cache: ResMut<RenderCache>) {
//!     cache.clear_all(); // Invalidate everything on pan
//! }
//!
//! fn handle_cursor_move(mut cache: ResMut<RenderCache>) {
//!     cache.clear_crosshair(); // Only invalidate crosshair overlay
//! }
//!
//! fn render_system(mut cache: ResMut<RenderCache>) {
//!     if cache.is_main_dirty() {
//!         // Expensive candlestick rendering...
//!         cache.mark_main_clean();
//!     }
//!     if cache.is_crosshair_dirty() {
//!         // Cheap crosshair rendering...
//!         cache.mark_crosshair_clean();
//!     }
//! }
//! ```

use bevy::prelude::*;
use std::time::{Duration, Instant};

/// Multi-tier cache state for selective invalidation.
///
/// Separates expensive main chart rendering from cheap overlays,
/// enabling efficient updates based on what changed. Each dirty flag
/// indicates that the corresponding render layer needs to be redrawn.
///
/// # Cache Tiers
///
/// | Tier | Cost | Invalidated By |
/// |------|------|----------------|
/// | `main_dirty` | High | Pan, zoom, resize, data updates |
/// | `crosshair_dirty` | Low | Cursor movement |
/// | `x_labels_dirty` | Medium | View changes, resize |
/// | `y_labels_dirty` | Medium | View changes, resize, price range changes |
///
/// # Performance Considerations
///
/// - The main cache should be invalidated sparingly, as it may contain
///   thousands of candlestick primitives.
/// - Crosshair updates are cheap and can happen on every frame.
/// - Label caches are moderately expensive due to text rendering.
#[derive(Resource, Debug, Clone)]
pub struct RenderCache {
    /// Whether the main candlestick layer needs redrawing (expensive).
    ///
    /// This cache contains the bulk of the rendering work: candlestick bodies,
    /// wicks, and any associated visual elements. Invalidate only when:
    /// - The view is panned or zoomed
    /// - The window is resized
    /// - New candlestick data arrives
    pub main_dirty: bool,

    /// Whether the crosshair overlay needs redrawing (cheap).
    ///
    /// The crosshair is a simple overlay consisting of two lines and
    /// optionally a tooltip. This can be redrawn frequently without
    /// significant performance impact.
    pub crosshair_dirty: bool,

    /// Whether the X-axis (time) labels need redrawing.
    ///
    /// Time labels change when the view is panned horizontally or
    /// zoomed, but not on cursor movement alone.
    pub x_labels_dirty: bool,

    /// Whether the Y-axis (price) labels need redrawing.
    ///
    /// Price labels change when the view is panned vertically, zoomed,
    /// or when the visible price range changes due to new data.
    pub y_labels_dirty: bool,

    /// Timestamp of the last cache invalidation.
    ///
    /// Useful for debugging and performance monitoring. Can be used
    /// to detect excessive invalidation patterns.
    pub last_invalidation: Option<Instant>,
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderCache {
    /// Creates a new cache with all layers marked as dirty.
    ///
    /// Initial state is fully dirty to ensure the first render
    /// draws all layers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache = RenderCache::new();
    /// assert!(cache.is_main_dirty());
    /// assert!(cache.is_crosshair_dirty());
    /// ```
    pub fn new() -> Self {
        Self {
            main_dirty: true,
            crosshair_dirty: true,
            x_labels_dirty: true,
            y_labels_dirty: true,
            last_invalidation: Some(Instant::now()),
        }
    }

    /// Clears all caches - used for pan, zoom, or resize operations.
    ///
    /// This is the most expensive invalidation as it forces a complete
    /// redraw of all layers. Use sparingly and only when the entire
    /// view has changed.
    ///
    /// # When to Use
    ///
    /// - User pans the chart (translation changed)
    /// - User zooms in/out (scaling changed)
    /// - Window or chart area is resized
    /// - Major data reset (e.g., switching symbols)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn handle_pan(mut cache: ResMut<RenderCache>) {
    ///     cache.clear_all();
    /// }
    /// ```
    pub fn clear_all(&mut self) {
        self.main_dirty = true;
        self.crosshair_dirty = true;
        self.x_labels_dirty = true;
        self.y_labels_dirty = true;
        self.last_invalidation = Some(Instant::now());
    }

    /// Clears only the crosshair cache - used for cursor movement.
    ///
    /// This preserves the expensive main chart cache while allowing
    /// the crosshair overlay to be updated. This is the preferred
    /// invalidation method for mouse movement events.
    ///
    /// # Performance
    ///
    /// This method is designed to be called on every cursor move event
    /// without causing performance issues, as the crosshair layer is
    /// very cheap to redraw.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn handle_cursor_move(mut cache: ResMut<RenderCache>, cursor_pos: Vec2) {
    ///     cache.clear_crosshair();
    ///     // Update cursor position...
    /// }
    /// ```
    pub fn clear_crosshair(&mut self) {
        self.crosshair_dirty = true;
        // Note: We intentionally do NOT update last_invalidation here
        // as crosshair updates are expected to be frequent and
        // tracking them would obscure more significant invalidations.
    }

    /// Clears only the main cache - used for data updates.
    ///
    /// This is used when new candlestick data arrives but the view
    /// hasn't changed. The crosshair and labels may remain valid
    /// if the visible range hasn't shifted.
    ///
    /// # When to Use
    ///
    /// - New candlestick data from WebSocket
    /// - Data modification (e.g., updating the latest candle)
    /// - Indicator recalculation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn handle_new_kline(mut cache: ResMut<RenderCache>, kline: Kline) {
    ///     // Store kline data...
    ///     cache.clear_main();
    /// }
    /// ```
    pub fn clear_main(&mut self) {
        self.main_dirty = true;
        self.last_invalidation = Some(Instant::now());
    }

    /// Clears only the label caches - used for view changes that don't affect candles.
    ///
    /// This is useful when labels need updating but the main chart content
    /// remains valid (rare case, but useful for label formatting changes).
    pub fn clear_labels(&mut self) {
        self.x_labels_dirty = true;
        self.y_labels_dirty = true;
        self.last_invalidation = Some(Instant::now());
    }

    /// Returns whether the main chart cache needs redrawing.
    ///
    /// Check this before performing expensive candlestick rendering.
    /// If false, the cached geometry can be reused.
    #[inline]
    pub fn is_main_dirty(&self) -> bool {
        self.main_dirty
    }

    /// Returns whether the crosshair cache needs redrawing.
    ///
    /// Check this before rendering the crosshair overlay.
    /// This will typically return true after any cursor movement.
    #[inline]
    pub fn is_crosshair_dirty(&self) -> bool {
        self.crosshair_dirty
    }

    /// Returns whether the X-axis labels need redrawing.
    #[inline]
    pub fn is_x_labels_dirty(&self) -> bool {
        self.x_labels_dirty
    }

    /// Returns whether the Y-axis labels need redrawing.
    #[inline]
    pub fn is_y_labels_dirty(&self) -> bool {
        self.y_labels_dirty
    }

    /// Returns whether any cache is dirty and needs redrawing.
    #[inline]
    pub fn is_any_dirty(&self) -> bool {
        self.main_dirty || self.crosshair_dirty || self.x_labels_dirty || self.y_labels_dirty
    }

    /// Marks the main chart cache as clean after rendering.
    ///
    /// Call this after successfully rendering the candlestick layer
    /// to prevent unnecessary redraws on subsequent frames.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn render_candles(mut cache: ResMut<RenderCache>) {
    ///     if cache.is_main_dirty() {
    ///         // Render candlesticks...
    ///         cache.mark_main_clean();
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn mark_main_clean(&mut self) {
        self.main_dirty = false;
    }

    /// Marks the crosshair cache as clean after rendering.
    ///
    /// Call this after successfully rendering the crosshair overlay.
    #[inline]
    pub fn mark_crosshair_clean(&mut self) {
        self.crosshair_dirty = false;
    }

    /// Marks the X-axis labels as clean after rendering.
    #[inline]
    pub fn mark_x_labels_clean(&mut self) {
        self.x_labels_dirty = false;
    }

    /// Marks the Y-axis labels as clean after rendering.
    #[inline]
    pub fn mark_y_labels_clean(&mut self) {
        self.y_labels_dirty = false;
    }

    /// Marks all caches as clean after a full render pass.
    ///
    /// Convenience method to mark all layers as clean in one call.
    pub fn mark_all_clean(&mut self) {
        self.main_dirty = false;
        self.crosshair_dirty = false;
        self.x_labels_dirty = false;
        self.y_labels_dirty = false;
    }

    /// Returns the time since the last significant invalidation.
    ///
    /// Returns `None` if no invalidation has occurred yet.
    /// Useful for debugging and performance monitoring.
    pub fn time_since_invalidation(&self) -> Option<Duration> {
        self.last_invalidation.map(|t| t.elapsed())
    }
}

/// Throttle state for frame-rate limited rendering.
///
/// Implements a simple throttling mechanism to prevent excessive redraws
/// when receiving high-frequency updates (e.g., real-time price feeds).
/// Updates are batched and rendered at a configurable interval.
///
/// # How It Works
///
/// 1. External systems call [`request_redraw()`](ThrottleState::request_redraw)
///    when data changes.
/// 2. The render system checks [`should_redraw()`](ThrottleState::should_redraw)
///    to determine if enough time has passed.
/// 3. After rendering, call [`mark_redrawn()`](ThrottleState::mark_redrawn)
///    to reset the throttle timer.
///
/// # Example
///
/// ```rust,ignore
/// fn handle_websocket_update(mut throttle: ResMut<ThrottleState>, kline: Kline) {
///     // Store data without immediate render...
///     throttle.request_redraw();
/// }
///
/// fn throttled_render(
///     mut throttle: ResMut<ThrottleState>,
///     mut cache: ResMut<RenderCache>,
/// ) {
///     if throttle.should_redraw() {
///         cache.clear_main();
///         // Render...
///         throttle.mark_redrawn();
///     }
/// }
/// ```
#[derive(Resource, Debug, Clone)]
pub struct ThrottleState {
    /// Whether a redraw has been requested since the last render.
    pub needs_redraw: bool,

    /// Timestamp of the last completed redraw.
    pub last_redraw: Instant,

    /// Minimum interval between redraws.
    ///
    /// Default is 100ms (10 FPS for data updates).
    /// This does not affect UI responsiveness for pan/zoom,
    /// only throttles high-frequency data updates.
    pub throttle_interval: Duration,
}

impl Default for ThrottleState {
    fn default() -> Self {
        Self {
            needs_redraw: false,
            last_redraw: Instant::now(),
            throttle_interval: Duration::from_millis(100),
        }
    }
}

impl ThrottleState {
    /// Creates a new throttle state with custom interval.
    ///
    /// # Arguments
    ///
    /// * `interval` - Minimum duration between redraws.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // 60 FPS throttling (~16ms)
    /// let throttle = ThrottleState::with_interval(Duration::from_millis(16));
    ///
    /// // 30 FPS throttling (~33ms)
    /// let throttle = ThrottleState::with_interval(Duration::from_millis(33));
    /// ```
    pub fn with_interval(interval: Duration) -> Self {
        Self {
            needs_redraw: false,
            last_redraw: Instant::now(),
            throttle_interval: interval,
        }
    }

    /// Requests a redraw on the next throttle tick.
    ///
    /// This does not immediately trigger a redraw; instead, it sets
    /// a flag that will be checked by [`should_redraw()`](Self::should_redraw).
    /// Multiple calls between redraws are coalesced into a single redraw.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn handle_price_update(mut throttle: ResMut<ThrottleState>) {
    ///     // Called potentially hundreds of times per second
    ///     throttle.request_redraw();
    /// }
    /// ```
    #[inline]
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Returns whether a redraw should occur now.
    ///
    /// Returns `true` if:
    /// 1. A redraw has been requested via [`request_redraw()`](Self::request_redraw), AND
    /// 2. The throttle interval has elapsed since the last redraw.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn render_system(throttle: Res<ThrottleState>) {
    ///     if throttle.should_redraw() {
    ///         // Perform render...
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn should_redraw(&self) -> bool {
        self.needs_redraw && self.last_redraw.elapsed() >= self.throttle_interval
    }

    /// Marks that a redraw has just completed.
    ///
    /// Resets the `needs_redraw` flag and updates the `last_redraw`
    /// timestamp. Call this after successfully completing a render pass.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn render_system(mut throttle: ResMut<ThrottleState>) {
    ///     if throttle.should_redraw() {
    ///         // Perform render...
    ///         throttle.mark_redrawn();
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn mark_redrawn(&mut self) {
        self.needs_redraw = false;
        self.last_redraw = Instant::now();
    }

    /// Returns the time remaining until the next possible redraw.
    ///
    /// Returns `Duration::ZERO` if the throttle interval has already elapsed.
    /// Useful for scheduling or debugging.
    pub fn time_until_next_redraw(&self) -> Duration {
        let elapsed = self.last_redraw.elapsed();
        if elapsed >= self.throttle_interval {
            Duration::ZERO
        } else {
            self.throttle_interval - elapsed
        }
    }

    /// Forces an immediate redraw by resetting the throttle timer to the past.
    ///
    /// Use sparingly for critical updates that cannot wait for the next
    /// throttle tick (e.g., user-initiated actions like reset view).
    pub fn force_immediate(&mut self) {
        self.needs_redraw = true;
        // Set last_redraw to a time far enough in the past
        self.last_redraw = Instant::now() - self.throttle_interval - Duration::from_millis(1);
    }

    /// Sets the throttle interval.
    ///
    /// Can be used to dynamically adjust throttling based on performance
    /// or user preferences.
    pub fn set_interval(&mut self, interval: Duration) {
        self.throttle_interval = interval;
    }

    /// Returns whether a redraw is pending (regardless of throttle).
    #[inline]
    pub fn is_redraw_pending(&self) -> bool {
        self.needs_redraw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_render_cache_new() {
        let cache = RenderCache::new();
        assert!(cache.is_main_dirty());
        assert!(cache.is_crosshair_dirty());
        assert!(cache.is_x_labels_dirty());
        assert!(cache.is_y_labels_dirty());
        assert!(cache.last_invalidation.is_some());
    }

    #[test]
    fn test_render_cache_clear_all() {
        let mut cache = RenderCache::new();
        cache.mark_all_clean();
        assert!(!cache.is_main_dirty());
        assert!(!cache.is_crosshair_dirty());

        cache.clear_all();
        assert!(cache.is_main_dirty());
        assert!(cache.is_crosshair_dirty());
        assert!(cache.is_x_labels_dirty());
        assert!(cache.is_y_labels_dirty());
    }

    #[test]
    fn test_render_cache_clear_crosshair() {
        let mut cache = RenderCache::new();
        cache.mark_all_clean();

        cache.clear_crosshair();
        assert!(!cache.is_main_dirty()); // Main should stay clean
        assert!(cache.is_crosshair_dirty());
    }

    #[test]
    fn test_render_cache_clear_main() {
        let mut cache = RenderCache::new();
        cache.mark_all_clean();

        cache.clear_main();
        assert!(cache.is_main_dirty());
        assert!(!cache.is_crosshair_dirty()); // Crosshair should stay clean
    }

    #[test]
    fn test_throttle_state_default() {
        let throttle = ThrottleState::default();
        assert!(!throttle.needs_redraw);
        assert_eq!(throttle.throttle_interval, Duration::from_millis(100));
    }

    #[test]
    fn test_throttle_request_and_check() {
        let mut throttle = ThrottleState::default();
        assert!(!throttle.should_redraw());

        throttle.request_redraw();
        // Immediately after request, throttle should block
        assert!(!throttle.should_redraw());
    }

    #[test]
    fn test_throttle_after_interval() {
        let mut throttle = ThrottleState::with_interval(Duration::from_millis(10));
        throttle.request_redraw();

        // Wait for throttle interval
        sleep(Duration::from_millis(15));
        assert!(throttle.should_redraw());

        throttle.mark_redrawn();
        assert!(!throttle.should_redraw());
    }

    #[test]
    fn test_throttle_force_immediate() {
        let mut throttle = ThrottleState::default();
        throttle.force_immediate();
        assert!(throttle.should_redraw());
    }
}
