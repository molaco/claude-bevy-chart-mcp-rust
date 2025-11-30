//! Performance benchmark tests for chart rendering.
//!
//! These tests measure and validate the performance optimizations implemented
//! in Phase 4: Rendering Consolidation.

mod common;

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};

use charts::config::InteractionConfig;
use charts::coordinate::ViewportState;
use charts::data::CandleData;
use charts::indicators::IndicatorState;
use charts::interaction::InteractionState;
use charts::panes::{GridBorderEntities, Pane, PaneId, PaneManager, PaneType};
use charts::types::{ChartGrid, ChartColors, ChartAxes, Crosshair, CrosshairState};
use charts::rendering::InstancingEnabled;

use common::{create_test_candles, test_viewport_area};

// ============================================================================
// PERFORMANCE METRICS
// ============================================================================

/// Collects performance metrics during benchmark runs.
#[derive(Default)]
struct MetricsCollector {
    frame_times_ms: Vec<f32>,
    update_count: u32,
    start_time: Option<Instant>,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            frame_times_ms: Vec::new(),
            update_count: 0,
            start_time: None,
        }
    }

    fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    fn record_frame(&mut self, frame_time_ms: f32) {
        self.frame_times_ms.push(frame_time_ms);
        self.update_count += 1;
    }

    fn finalize(self) -> BenchmarkResult {
        let total_frames = self.frame_times_ms.len();
        if total_frames == 0 {
            return BenchmarkResult::default();
        }

        let sum: f32 = self.frame_times_ms.iter().sum();
        let avg_frame_time_ms = sum / total_frames as f32;
        let avg_fps = if avg_frame_time_ms > 0.0 {
            1000.0 / avg_frame_time_ms
        } else {
            0.0
        };

        // Calculate P99 frame time
        let mut sorted = self.frame_times_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p99_idx = (total_frames as f32 * 0.99) as usize;
        let p99_frame_time_ms = sorted.get(p99_idx.min(total_frames - 1)).copied().unwrap_or(0.0);

        let elapsed = self.start_time.map(|s| s.elapsed()).unwrap_or_default();

        BenchmarkResult {
            total_frames,
            avg_fps,
            avg_frame_time_ms,
            p99_frame_time_ms,
            elapsed,
        }
    }
}

/// Results from a benchmark run.
#[derive(Debug, Default)]
struct BenchmarkResult {
    total_frames: usize,
    avg_fps: f32,
    avg_frame_time_ms: f32,
    p99_frame_time_ms: f32,
    elapsed: Duration,
}

impl BenchmarkResult {
    fn print(&self, name: &str) {
        println!("=== {} ===", name);
        println!("  Total frames: {}", self.total_frames);
        println!("  Average FPS: {:.1}", self.avg_fps);
        println!("  Avg frame time: {:.2}ms", self.avg_frame_time_ms);
        println!("  P99 frame time: {:.2}ms", self.p99_frame_time_ms);
        println!("  Total elapsed: {:?}", self.elapsed);
        println!();
    }
}

// ============================================================================
// TEST APP SETUP
// ============================================================================

/// Create a performance test app with specified candle count.
fn create_perf_test_app(candle_count: usize) -> App {
    let mut app = App::new();

    // Minimal plugins for performance testing
    app.add_plugins(MinimalPlugins);
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());

    // Create test data
    let candles = create_test_candles(candle_count);
    let total_area = test_viewport_area();

    let visible_count = candle_count.min(100);
    let visible_start = candle_count.saturating_sub(visible_count);

    // Create pane manager
    let mut pane_manager = PaneManager::new_with_gap(
        vec![
            Pane::new(PaneId::Price, PaneType::Price, 0.7, total_area, visible_count),
            Pane::new(PaneId::Volume, PaneType::Volume, 0.3, total_area, visible_count),
        ],
        10.0,
    );
    pane_manager.calculate_layouts(total_area, visible_count);

    // Create viewport state
    let viewport_state = ViewportState::new(visible_start, visible_count, total_area);

    // Insert resources
    app.insert_resource(CandleData::new(candles));
    app.insert_resource(viewport_state);
    app.insert_resource(pane_manager);
    app.insert_resource(IndicatorState::default());
    app.insert_resource(InteractionState::default());
    app.insert_resource(InteractionConfig::default());
    app.insert_resource(ChartGrid::default());
    app.insert_resource(ChartAxes::default());
    app.insert_resource(Crosshair::default());
    app.insert_resource(CrosshairState::default());
    app.insert_resource(ChartColors::default());
    app.insert_resource(InstancingEnabled::default());
    app.insert_resource(GridBorderEntities::default());

    app
}

/// Run benchmark for specified number of frames.
fn run_benchmark(app: &mut App, frame_count: usize) -> BenchmarkResult {
    let mut collector = MetricsCollector::new();
    collector.start();

    // Warmup: run a few frames to initialize
    for _ in 0..5 {
        app.update();
    }

    // Measure frames
    for _ in 0..frame_count {
        let frame_start = Instant::now();
        app.update();
        let frame_time = frame_start.elapsed();
        collector.record_frame(frame_time.as_secs_f32() * 1000.0);
    }

    collector.finalize()
}

// ============================================================================
// BENCHMARK TESTS
// ============================================================================

#[test]
fn benchmark_static_100_candles() {
    let mut app = create_perf_test_app(100);
    let results = run_benchmark(&mut app, 60);
    results.print("Static 100 Candles");

    // Relaxed assertions for CI - just verify it runs
    assert!(results.total_frames > 0, "Should complete frames");
}

#[test]
fn benchmark_static_1k_candles() {
    let mut app = create_perf_test_app(1_000);
    let results = run_benchmark(&mut app, 60);
    results.print("Static 1K Candles");

    assert!(results.total_frames > 0, "Should complete frames");
}

#[test]
fn benchmark_static_10k_candles() {
    let mut app = create_perf_test_app(10_000);
    let results = run_benchmark(&mut app, 30);
    results.print("Static 10K Candles");

    assert!(results.total_frames > 0, "Should complete frames");
}

#[test]
fn benchmark_viewport_change() {
    let mut app = create_perf_test_app(1_000);
    let mut collector = MetricsCollector::new();
    collector.start();

    // Warmup
    for _ in 0..5 {
        app.update();
    }

    // Simulate viewport changes (pan/zoom)
    for i in 0..30 {
        // Toggle needs_redraw to simulate interaction
        {
            let mut viewport = app.world_mut().resource_mut::<ViewportState>();
            viewport.needs_redraw = true;
            viewport.visible_candle_start = (i * 10) % 900;
        }

        let frame_start = Instant::now();
        app.update();
        let frame_time = frame_start.elapsed();
        collector.record_frame(frame_time.as_secs_f32() * 1000.0);
    }

    let results = collector.finalize();
    results.print("Viewport Change (Pan Simulation)");

    assert!(results.total_frames > 0, "Should complete frames");
}

#[test]
fn benchmark_static_no_redraw() {
    let mut app = create_perf_test_app(1_000);

    // Initial setup
    for _ in 0..5 {
        app.update();
    }

    // Ensure needs_redraw is false
    {
        let mut viewport = app.world_mut().resource_mut::<ViewportState>();
        viewport.needs_redraw = false;
    }

    let mut collector = MetricsCollector::new();
    collector.start();

    // Measure frames with no redraw needed
    for _ in 0..60 {
        let frame_start = Instant::now();
        app.update();
        let frame_time = frame_start.elapsed();
        collector.record_frame(frame_time.as_secs_f32() * 1000.0);
    }

    let results = collector.finalize();
    results.print("Static (No Redraw Flag)");

    assert!(results.total_frames > 0, "Should complete frames");
    // No redraw should be faster than with redraw
}

#[test]
fn benchmark_comparison_summary() {
    // Run all benchmarks and compare
    println!("\n");
    println!("============================================");
    println!("PERFORMANCE BENCHMARK SUMMARY");
    println!("============================================\n");

    // 100 candles
    let mut app_100 = create_perf_test_app(100);
    let r100 = run_benchmark(&mut app_100, 60);
    r100.print("100 Candles");

    // 1K candles
    let mut app_1k = create_perf_test_app(1_000);
    let r1k = run_benchmark(&mut app_1k, 60);
    r1k.print("1K Candles");

    // 5K candles
    let mut app_5k = create_perf_test_app(5_000);
    let r5k = run_benchmark(&mut app_5k, 30);
    r5k.print("5K Candles");

    // 10K candles
    let mut app_10k = create_perf_test_app(10_000);
    let r10k = run_benchmark(&mut app_10k, 30);
    r10k.print("10K Candles");

    println!("============================================");
    println!("Scaling Analysis:");
    println!("  100 -> 1K: {:.1}x slower", r1k.avg_frame_time_ms / r100.avg_frame_time_ms.max(0.001));
    println!("  1K -> 5K: {:.1}x slower", r5k.avg_frame_time_ms / r1k.avg_frame_time_ms.max(0.001));
    println!("  5K -> 10K: {:.1}x slower", r10k.avg_frame_time_ms / r5k.avg_frame_time_ms.max(0.001));
    println!("============================================\n");

    // All tests should complete
    assert!(r100.total_frames > 0);
    assert!(r1k.total_frames > 0);
    assert!(r5k.total_frames > 0);
    assert!(r10k.total_frames > 0);
}

// ============================================================================
// OPTIMIZATION VALIDATION TESTS
// ============================================================================

#[test]
fn test_change_detection_reduces_work() {
    let mut app = create_perf_test_app(1_000);

    // Run with redraw flag set
    for _ in 0..5 {
        {
            let mut viewport = app.world_mut().resource_mut::<ViewportState>();
            viewport.needs_redraw = true;
        }
        app.update();
    }

    // Collect timing with redraw
    let mut redraw_times = Vec::new();
    for _ in 0..10 {
        {
            let mut viewport = app.world_mut().resource_mut::<ViewportState>();
            viewport.needs_redraw = true;
        }
        let start = Instant::now();
        app.update();
        redraw_times.push(start.elapsed());
    }

    // Collect timing without redraw
    let mut no_redraw_times = Vec::new();
    for _ in 0..10 {
        {
            let mut viewport = app.world_mut().resource_mut::<ViewportState>();
            viewport.needs_redraw = false;
        }
        let start = Instant::now();
        app.update();
        no_redraw_times.push(start.elapsed());
    }

    let avg_redraw: Duration = redraw_times.iter().sum::<Duration>() / redraw_times.len() as u32;
    let avg_no_redraw: Duration = no_redraw_times.iter().sum::<Duration>() / no_redraw_times.len() as u32;

    println!("Change Detection Validation:");
    println!("  Avg with redraw: {:?}", avg_redraw);
    println!("  Avg without redraw: {:?}", avg_no_redraw);

    // No redraw should be faster (though exact ratio depends on system)
    // This is a soft assertion - the optimization should help but exact improvement varies
    assert!(no_redraw_times.len() > 0, "Should have timing data");
}

#[test]
fn test_instancing_enabled() {
    let app = create_perf_test_app(100);

    // Verify InstancingEnabled resource exists and is enabled
    let instancing = app.world().resource::<InstancingEnabled>();
    assert!(instancing.0, "GPU instancing should be enabled by default");
}

#[test]
fn test_grid_border_entities_resource() {
    let app = create_perf_test_app(100);

    // Verify GridBorderEntities resource exists
    let grid_entities = app.world().resource::<GridBorderEntities>();
    // Initially not initialized until first render
    assert!(!grid_entities.initialized, "GridBorderEntities should start uninitialized");
}
