//! GPU instanced rendering plugin for candlestick charts
//!
//! This module provides a Bevy plugin that implements GPU instanced rendering
//! for candlestick charts. It manages the render pipeline, resources, and
//! extraction/preparation systems that run in the render world.
//!
//! # Architecture
//! - Main world: Contains `InstancingEnabled` resource to toggle the feature
//! - Render world: Contains extraction, preparation, and rendering resources
//! - Pipeline: Created in `finish()` and cached for reuse
//!
//! # Resources
//! - `ExtractedCandlesInstanced`: Holds extracted candle instance data from main world
//! - `CandleRenderData`: Manages GPU buffers and bind groups
//! - `CandlePipeline`: Cached render pipeline and bind group layout
//! - `InstancingEnabled`: Toggle for enabling/disabling GPU instancing

use super::instancing::{CandleInstance, CandleStyle, ChartConfig, ViewUniform};
use super::triple_buffer::CandleTripleBuffer;
use crate::types::{CandleData, ChartColors, PaneManager, ViewportState};
use std::time::Instant;
use bevy::asset::AssetServer;
use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{
        NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
    },
    render_resource::{
        BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingType, BlendState,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, FragmentState, FrontFace, LoadOp, MultisampleState,
        Operations, PipelineCache, PolygonMode, PrimitiveState, PrimitiveTopology,
        RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages,
        StoreOp, TextureFormat, VertexState,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    view::ViewTarget,
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
};
use bevy::window::Window;

/// Resource containing extracted candlestick instance data
///
/// This resource lives in the render world and is populated during the extract
/// phase. It contains all the data needed to render candlesticks using GPU instancing.
///
/// # Change Detection
/// The `instances_changed` flag is set when any of these conditions occur:
/// - Time range changes (zoom/pan)
/// - Candle count changes (new data loaded)
/// - Viewport size changes (window resize)
///
/// # State Tracking
/// The `last_*` fields are used to detect changes between frames and avoid
/// unnecessary GPU buffer updates.
#[derive(Resource, Default)]
pub struct ExtractedCandlesInstanced {
    /// Instance data for all visible candlesticks
    pub instances: Vec<CandleInstance>,

    /// True if the entire chart needs to be redrawn (forces full update)
    pub needs_redraw: bool,

    /// True if instance data has changed since last frame
    pub instances_changed: bool,

    /// Current viewport width in pixels
    pub viewport_width: f32,

    /// Current viewport height in pixels
    pub viewport_height: f32,

    // State tracking for change detection
    /// Last rendered time range start (for detecting zoom/pan)
    pub last_time_start: i64,

    /// Last rendered time range end (for detecting zoom/pan)
    pub last_time_end: i64,

    /// Last rendered candle count (for detecting data changes)
    pub last_candle_count: usize,

    /// Last rendered viewport width (for detecting resizes)
    pub last_viewport_width: f32,

    /// Last rendered viewport height (for detecting resizes)
    pub last_viewport_height: f32,

    // Theme colors (linear RGBA)
    /// Bullish candle color in linear RGBA (typically green)
    pub bull_color: [f32; 4],

    /// Bearish candle color in linear RGBA (typically red)
    pub bear_color: [f32; 4],

    /// Wick color in linear RGBA (typically gray)
    pub wick_color: [f32; 4],
}

/// Resource containing the cached render pipeline and bind group layout
///
/// The pipeline is created during the `finish()` phase after the render device
/// is available. It is cached to avoid recreating the pipeline every frame.
///
/// # Pipeline State
/// - Vertex shader: Processes CandleInstance attributes
/// - Fragment shader: Applies colors based on is_bullish flag
/// - Blend mode: Alpha blending for transparency
/// - Primitive topology: Triangle list (instanced quads)
///
/// # Bind Group Layout
/// The layout defines the structure of the bind group:
/// - Group 0, Binding 0: View uniform (ViewUniform struct)
#[derive(Resource)]
pub struct CandlePipeline {
    /// Cached pipeline ID from the render pipeline cache
    pub pipeline_id: CachedRenderPipelineId,

    /// Bind group layout for view uniform and resources
    pub bind_group_layout: BindGroupLayout,
}

/// Resource to toggle GPU instanced rendering on/off
///
/// This resource lives in the main world and allows runtime switching between
/// instanced and non-instanced rendering paths. Useful for:
/// - Performance testing and comparisons
/// - Debugging rendering issues
/// - Fallback for platforms with limited instancing support
///
/// # Default
/// Instancing is enabled by default for best performance.
#[derive(Resource)]
pub struct InstancingEnabled(pub bool);

impl Default for InstancingEnabled {
    fn default() -> Self {
        Self(true) // Enabled by default
    }
}

/// Render graph label for the candlestick instanced rendering node
///
/// This label is used to identify the candlestick rendering node in the
/// render graph, allowing other nodes to reference it for dependency ordering.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct CandlestickInstancedLabel;

/// Render graph node that executes the GPU instanced candlestick rendering
///
/// This node runs in the render graph and is responsible for:
/// - Retrieving the render pipeline and GPU buffers
/// - Creating a render pass
/// - Executing the instanced draw call
///
/// # Execution Order
/// The node runs after `MainTransparentPass` and before `EndMainPass` in the
/// Core2d render graph, ensuring candlesticks are drawn on top of the base
/// content but before post-processing effects.
///
/// # Early Returns
/// The node will skip rendering if:
/// - No instances are present (instance_count == 0)
/// - GPU buffers are not ready (None)
/// - Pipeline is still compiling
#[derive(Default)]
pub struct CandlestickNode;

impl ViewNode for CandlestickNode {
    type ViewQuery = &'static ViewTarget;

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        view_target: QueryItem<'w, 'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // 1. Get triple-buffered resources directly
        let triple_buffer = world.resource::<CandleTripleBuffer>();

        // Get the slot that was written in the prepare phase of THIS frame.
        // Uses current_write_slot which was set by begin_frame() before complete_frame() advanced the counter.
        let render_slot_index = triple_buffer.resources.render_slot_index();
        let slot = &triple_buffer.resources.slots[render_slot_index];

        // Skip if no instances in this slot
        if slot.instance_count == 0 {
            return Ok(());
        }

        // Get buffers and bind group from the correct slot (return early if not ready)
        let (Some(instance_buffer), Some(bind_group)) =
            (&slot.instance_buffer, &slot.bind_group)
        else {
            // Debug: log when slot is not ready
            static SKIP_LOGGED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !SKIP_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                println!(
                    "[Render] SKIP: slot {} not ready (buffer={}, bind_group={})",
                    render_slot_index,
                    slot.instance_buffer.is_some(),
                    slot.bind_group.is_some()
                );
            }
            return Ok(());
        };

        // 2. Get pipeline (check compilation status)
        let Some(pipeline_resource) = world.get_resource::<CandlePipeline>() else {
            return Ok(());
        };

        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_resource.pipeline_id)
        else {
            return Ok(()); // Pipeline still compiling
        };

        // 3. Create render pass
        let color_attachment = view_target.get_color_attachment();

        let mut render_pass =
            render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("candlestick_instanced_pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: color_attachment.view,
                        resolve_target: color_attachment.resolve_target,
                        ops: Operations {
                            load: LoadOp::Load, // Preserve existing content
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

        // 4. Set pipeline and bindings (using triple buffer slot's bind group)
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, *instance_buffer.slice(..));

        // 5. Draw: 12 vertices per candle, N instances
        // Use the slot's instance_count (not the global one) to match the data in this slot
        const VERTICES_PER_CANDLE: u32 = 12;
        let instance_count = slot.instance_count;
        render_pass.draw(0..VERTICES_PER_CANDLE, 0..instance_count);

        // Debug: track render node calls per frame
        static RENDER_FRAME_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        static LAST_LOGGED_FRAME: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);

        let render_frame = RENDER_FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let last_logged = LAST_LOGGED_FRAME.load(std::sync::atomic::Ordering::Relaxed);

        // Log every 60 frames or first 10 frames
        if render_frame < 10 || (render_frame - last_logged) >= 60 {
            LAST_LOGGED_FRAME.store(render_frame, std::sync::atomic::Ordering::Relaxed);
            println!(
                "[Render] Frame {}: slot={}, instances={}, frame_count={}",
                render_frame,
                render_slot_index,
                instance_count,
                triple_buffer.resources.frame_count
            );
        }

        Ok(())
    }
}

// ============================================================================
// PREPARE SYSTEM
// ============================================================================

/// Build an orthographic projection matrix
///
/// Maps screen coordinates (0,0)-(width,height) to clip space (-1,-1)-(1,1)
/// This is used by the vertex shader to transform world positions to clip space.
///
/// # Arguments
/// * `width` - Viewport width in pixels
/// * `height` - Viewport height in pixels
///
/// # Returns
/// 4x4 column-major projection matrix (centered, like Bevy's 2D camera)
fn build_orthographic_matrix(width: f32, height: f32) -> [[f32; 4]; 4] {
    // Bevy's 2D camera uses centered coordinates: (0,0) is screen center
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let left = -half_width;
    let right = half_width;
    let bottom = -half_height;
    let top = half_height;
    let near = -1.0;
    let far = 1.0;

    [
        [2.0 / (right - left), 0.0, 0.0, 0.0],
        [0.0, 2.0 / (top - bottom), 0.0, 0.0],
        [0.0, 0.0, -2.0 / (far - near), 0.0],
        [
            -(right + left) / (right - left),
            -(top + bottom) / (top - bottom),
            -(far + near) / (far - near),
            1.0,
        ],
    ]
}

/// Prepare GPU buffers and bind groups for candlestick rendering
///
/// This system runs in the Render schedule (PrepareResources set) and is responsible
/// for creating and updating GPU buffers based on extracted data. It implements
/// triple buffering to eliminate CPU/GPU synchronization stalls.
///
/// # Triple Buffering Strategy
/// - Uses 3 rotating instance buffers to avoid CPU/GPU contention
/// - Each frame writes to a different buffer slot
/// - GPU reads from a buffer written 2 frames ago (guaranteed complete)
///
/// # Buffer Management
/// - Instance buffers: Triple-buffered, one per slot
/// - View uniform buffer: Shared across all slots (updated when viewport changes)
/// - Config buffer: Static, created once
/// - Bind groups: One per slot (references slot's instance buffer + shared uniforms)
///
/// # Performance Optimizations
/// - Tracks buffer capacity per slot to avoid unnecessary reallocations
/// - Only writes to GPU when data actually changed
/// - Fence tracking ensures safe buffer reuse
fn prepare_candles_instanced(
    mut triple_buffer: ResMut<CandleTripleBuffer>,
    extracted: Res<ExtractedCandlesInstanced>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Option<Res<CandlePipeline>>,
) {
    let Some(pipeline) = pipeline else {
        triple_buffer.resources.skip_frame();
        return;
    };

    // Early return if no data
    if extracted.instances.is_empty() {
        triple_buffer.resources.instance_count = 0;
        triple_buffer.resources.skip_frame();
        return;
    }

    // ========================================================================
    // PRE-ALLOCATION (First frame only - allocate all 3 slots upfront)
    // ========================================================================

    let instance_data: &[u8] = bytemuck::cast_slice(&extracted.instances);
    let required_size = instance_data.len();

    // Pre-allocate all 3 slots on first frame to avoid hitches during rotation
    // Use growth factor for extra headroom to reduce future reallocations
    if !triple_buffer.resources.is_preallocated() && required_size > 0 {
        // Calculate capacity with initial growth factor (1.5^2 = 2.25x headroom)
        let initial_capacity = super::triple_buffer::calculate_initial_capacity(required_size);

        #[cfg(debug_assertions)]
        println!(
            "[Candle] Pre-allocating 3 instance buffers: {} bytes data, {} bytes capacity ({}x headroom)",
            required_size,
            initial_capacity,
            initial_capacity as f32 / required_size as f32
        );

        // Create a zero-filled buffer at full capacity, then write actual data
        let mut padded_data = vec![0u8; initial_capacity];
        padded_data[..required_size].copy_from_slice(instance_data);

        // Pre-allocate all slots with same data and same instance count
        // This ensures gpu_read_index() will find valid data from the start
        let initial_instance_count = extracted.instances.len() as u32;
        for i in 0..super::triple_buffer::BUFFER_COUNT {
            let slot = &mut triple_buffer.resources.slots[i];
            if slot.instance_buffer.is_none() {
                let label = match i {
                    0 => "candlestick_instance_buffer_slot0",
                    1 => "candlestick_instance_buffer_slot1",
                    _ => "candlestick_instance_buffer_slot2",
                };
                slot.instance_buffer = Some(render_device.create_buffer_with_data(
                    &BufferInitDescriptor {
                        label: Some(label),
                        contents: &padded_data,
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    },
                ));
                slot.buffer_capacity = initial_capacity;
                // Set instance count for all slots so gpu_read_index() finds valid data
                slot.instance_count = initial_instance_count;
            }
        }

        // Record allocation stats
        triple_buffer.resources.record_initial_allocation(initial_capacity);
    }

    // Track peak instance count for stats
    triple_buffer.resources.track_instance_count(extracted.instances.len() as u32);

    // ========================================================================
    // TRIPLE BUFFER SYNCHRONIZATION FLOW
    // ========================================================================

    // Step 1: Begin frame - acquire current slot and update fence states
    // Time the begin_frame() call to measure any fence wait overhead
    let begin_time = Instant::now();
    let frame_ctx = triple_buffer.resources.begin_frame();
    let fence_wait_us = begin_time.elapsed().as_micros() as u64;
    let slot_index = frame_ctx.slot_index;

    // Record debug stats
    triple_buffer.resources.debug_begin_frame();
    triple_buffer.resources.debug_record_fence_wait(fence_wait_us);
    triple_buffer.resources.debug_record_instance_count(extracted.instances.len() as u32);

    // Debug: Log buffer rotation for first 10 frames
    #[cfg(debug_assertions)]
    if frame_ctx.frame_number < 10 {
        let rotation = triple_buffer.resources.rotation_state();
        println!(
            "[Candle] Frame {}: Write slot {}, GPU reads slot {}, InFlight slot {} | {} instances | fence_wait={}µs",
            frame_ctx.frame_number,
            rotation.write_slot,
            rotation.gpu_read_slot,
            rotation.in_flight_slot,
            extracted.instances.len(),
            fence_wait_us
        );
    }

    // Step 2: Write instance data to current slot's buffer
    // Track reallocation info outside the borrow scope
    let mut reallocation_info: Option<(usize, usize)> = None;

    {
        let slot = &mut triple_buffer.resources.slots[slot_index];

        // Only recreate buffer if capacity insufficient
        if slot.needs_reallocation(required_size) {
            // Calculate new capacity with growth factor (1.5x)
            let new_capacity = super::triple_buffer::calculate_grown_capacity(required_size);
            let old_capacity = slot.buffer_capacity;

            #[cfg(debug_assertions)]
            println!(
                "[Candle] Reallocating slot {}: {} -> {} bytes (required: {})",
                slot_index, old_capacity, new_capacity, required_size
            );

            // Create padded buffer at full capacity
            let mut padded_data = vec![0u8; new_capacity];
            padded_data[..required_size].copy_from_slice(instance_data);

            let label = match slot_index {
                0 => "candlestick_instance_buffer_slot0",
                1 => "candlestick_instance_buffer_slot1",
                _ => "candlestick_instance_buffer_slot2",
            };
            slot.instance_buffer = Some(render_device.create_buffer_with_data(
                &BufferInitDescriptor {
                    label: Some(label),
                    contents: &padded_data,
                    usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                },
            ));

            slot.buffer_capacity = new_capacity;
            slot.invalidate_bind_group(); // Buffer changed, need new bind group

            // Store reallocation info for stats recording after borrow ends
            reallocation_info = Some((old_capacity, new_capacity));
        } else {
            // ALWAYS write to GPU - each slot needs current data because this slot
            // may have stale data from 3 frames ago when it was last written.
            // Without this, stopping a zoom causes flickering as we render old data.
            if let Some(buffer) = &slot.instance_buffer {
                render_queue.write_buffer(buffer, 0, instance_data);
            }
        }
    }

    // Record reallocation stats after slot borrow is released
    if let Some((old_capacity, new_capacity)) = reallocation_info {
        triple_buffer.resources.record_slot_reallocation(old_capacity, new_capacity);
        triple_buffer.resources.debug_record_reallocation();
    }

    // ========================================================================
    // PER-SLOT VIEW UNIFORM (Triple-buffered to avoid CPU/GPU contention)
    // ========================================================================

    // Build view uniform data
    let view_uniform = ViewUniform {
        view_proj: build_orthographic_matrix(
            extracted.viewport_width,
            extracted.viewport_height,
        ),
        viewport: [
            0.0,
            0.0,
            extracted.viewport_width,
            extracted.viewport_height,
        ],
        bull_color: extracted.bull_color,
        bear_color: extracted.bear_color,
        wick_color: extracted.wick_color,
    };

    let uniform_data: &[u8] = bytemuck::bytes_of(&view_uniform);
    let uniform_size = uniform_data.len();

    // Write to current slot's uniform buffer (each slot has its own)
    {
        let slot = &mut triple_buffer.resources.slots[slot_index];

        if slot.needs_uniform_reallocation(uniform_size) {
            let label = match slot_index {
                0 => "candlestick_view_uniform_slot0",
                1 => "candlestick_view_uniform_slot1",
                _ => "candlestick_view_uniform_slot2",
            };
            slot.view_uniform_buffer = Some(render_device.create_buffer_with_data(
                &BufferInitDescriptor {
                    label: Some(label),
                    contents: uniform_data,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                },
            ));
            slot.uniform_buffer_capacity = uniform_size;
            slot.invalidate_bind_group(); // Buffer changed, need new bind group
        } else if let Some(buffer) = &slot.view_uniform_buffer {
            // Always write uniform data to current slot's buffer
            render_queue.write_buffer(buffer, 0, uniform_data);
        }
    }

    // Update tracking state for change detection
    triple_buffer.resources.update_view_tracking(
        extracted.viewport_width,
        extracted.viewport_height,
        extracted.bull_color,
        extracted.bear_color,
        extracted.wick_color,
    );

    // ========================================================================
    // SHARED CONFIG BUFFER (Static, never changes)
    // ========================================================================

    if triple_buffer.resources.config_buffer.is_none() {
        let config = ChartConfig::default();
        let config_data: &[u8] = bytemuck::bytes_of(&config);
        triple_buffer.resources.config_buffer = Some(render_device.create_buffer_with_data(
            &BufferInitDescriptor {
                label: Some("candlestick_config_buffer"),
                contents: config_data,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            },
        ));
        triple_buffer.resources.invalidate_all_bind_groups();
    }

    // ========================================================================
    // BIND GROUP (Per-slot, references slot's own uniform buffer)
    // ========================================================================

    let needs_bind_group = {
        let slot = &triple_buffer.resources.slots[slot_index];
        (slot.bind_group.is_none() || !slot.bind_group_valid)
            && slot.instance_buffer.is_some()
            && slot.view_uniform_buffer.is_some()
            && triple_buffer.resources.config_buffer.is_some()
    };

    if needs_bind_group {
        let slot = &triple_buffer.resources.slots[slot_index];
        let view_buffer = slot.view_uniform_buffer.as_ref().unwrap();
        let config_buffer = triple_buffer.resources.config_buffer.as_ref().unwrap();

        let label = match slot_index {
            0 => "candlestick_bind_group_slot0",
            1 => "candlestick_bind_group_slot1",
            _ => "candlestick_bind_group_slot2",
        };

        let bind_group = render_device.create_bind_group(
            Some(label),
            &pipeline.bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: view_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        );

        let slot = &mut triple_buffer.resources.slots[slot_index];
        slot.bind_group = Some(bind_group);
        slot.bind_group_valid = true;
    }

    // Update instance count - store in BOTH slot and global for backwards compatibility
    let instance_count = extracted.instances.len() as u32;
    triple_buffer.resources.instance_count = instance_count;
    triple_buffer.resources.slots[slot_index].instance_count = instance_count;

    // Complete debug frame tracking
    triple_buffer.resources.debug_end_frame();

    // Step 3: Complete frame - mark slot as submitted, advance frame counter
    triple_buffer.resources.complete_frame();
}

// ============================================================================
// EXTRACT SYSTEM
// ============================================================================

/// Convert sRGB color to linear RGBA array for GPU upload
///
/// The render target uses Rgba8UnormSrgb format which expects linear input
/// and applies sRGB gamma correction automatically on output.
fn color_to_array(color: &Color) -> [f32; 4] {
    let linear = color.to_linear();
    [linear.red, linear.green, linear.blue, linear.alpha]
}

/// Extract candlestick data from main world to render world
///
/// This system runs in the ExtractSchedule and populates ExtractedCandlesInstanced
/// with instance data for all visible candlesticks. It implements change detection
/// to skip expensive rebuilds when nothing has changed.
///
/// # Early Returns
/// - If instancing is disabled
/// - If no candles exist
/// - If no price pane exists
/// - If price range is invalid
/// - If state hasn't changed (optimization)
fn extract_candles_instanced(
    mut extracted: ResMut<ExtractedCandlesInstanced>,
    candle_data: Extract<Res<CandleData>>,
    viewport_state: Extract<Res<ViewportState>>,
    pane_manager: Extract<Res<PaneManager>>,
    instancing_enabled: Extract<Res<InstancingEnabled>>,
    chart_colors: Extract<Res<ChartColors>>,
    windows: Extract<Query<&Window>>,
) {
    // Reset flags at start of each frame
    extracted.needs_redraw = false;
    extracted.instances_changed = false;

    // Extract colors from theme
    extracted.bull_color = color_to_array(&chart_colors.bull_candle);
    extracted.bear_color = color_to_array(&chart_colors.bear_candle);
    extracted.wick_color = color_to_array(&chart_colors.wick);

    // Early return if instancing disabled
    if !instancing_enabled.0 {
        extracted.instances.clear();
        return;
    }

    // Early return if no data
    if candle_data.candles.is_empty() {
        return;
    }

    // Get window size for viewport tracking
    let (viewport_width, viewport_height) = if let Ok(window) = windows.single() {
        (window.width(), window.height())
    } else {
        (extracted.viewport_width, extracted.viewport_height)
    };

    // Find the Price pane
    let price_pane = match pane_manager.price_pane() {
        Some(pane) => pane,
        None => return,
    };

    // Get visible candle range from ViewportState
    let start = viewport_state.visible_candle_start;
    let count = viewport_state.visible_candle_count;
    let end = (start + count).min(candle_data.candles.len());

    if start >= end {
        return;
    }

    let visible_candles = &candle_data.candles[start..end];

    // Use ChartSpace's price range (same as old rendering system)
    let price_min = price_pane.space.visible_price_min;
    let price_max = price_pane.space.visible_price_max;
    let price_range = price_max - price_min;

    if price_range <= 0.0 {
        return;
    }

    // Check if state changed - skip expensive rebuild if nothing changed
    let state_unchanged = extracted.last_time_start == candle_data.candles[start].time
        && extracted.last_time_end == candle_data.candles[end - 1].time
        && extracted.last_candle_count == visible_candles.len()
        && (extracted.last_viewport_width - viewport_width).abs() < 1.0_f32
        && (extracted.last_viewport_height - viewport_height).abs() < 1.0_f32
        && !extracted.instances.is_empty();

    if state_unchanged {
        // Nothing changed, keep existing data
        extracted.needs_redraw = true; // Still need to render
        return;
    }

    // State changed - rebuild instance buffer
    extracted.instances.clear();
    extracted.viewport_width = viewport_width;
    extracted.viewport_height = viewport_height;

    let viewport = &price_pane.space.viewport;

    // Calculate candle width and body width (use visible_candle_count like old system)
    let visible_count = viewport_state.visible_candle_count;
    let candle_width = viewport.width() / visible_count.max(1) as f32;
    let body_width = candle_width * crate::config::CANDLE_BODY_WIDTH_RATIO;

    // Determine adaptive visualization style based on visible candle count
    let candle_style = CandleStyle::from_count(visible_count);

    // Coordinate transformation functions (match ChartSpace::to_world exactly)
    let price_to_y = |price: f64| -> f32 {
        let price_percent = if price_range > 0.0 {
            (price as f32 - price_min) / price_range
        } else {
            0.5
        };
        viewport.min.y + price_percent * viewport.height()
    };

    let time_to_x = |candle_index: usize| -> f32 {
        let candle_offset = candle_index.saturating_sub(start);
        let x_percent = candle_offset as f32 / visible_count as f32;
        viewport.min.x + x_percent * viewport.width()
    };

    // Create instances for all visible candles
    extracted.instances.reserve(visible_candles.len());

    let mut bull_count = 0;
    let mut bear_count = 0;

    for (idx, candle) in visible_candles.iter().enumerate() {
        let candle_index = start + idx;
        let x_pos = time_to_x(candle_index);
        let open_y = price_to_y(candle.open);
        let high_y = price_to_y(candle.high);
        let low_y = price_to_y(candle.low);
        let close_y = price_to_y(candle.close);
        let is_bullish = candle.close >= candle.open;

        if is_bullish {
            bull_count += 1;
        } else {
            bear_count += 1;
        }

        // Adaptive visualization: for HighLowBar style, body shows high/low
        // The wick (drawn behind body) becomes invisible when body covers full range
        let (instance_open, instance_close) = if candle_style.body_shows_high_low() {
            // Body spans high to low, color still based on original direction
            (high_y, low_y)
        } else {
            // Normal: body shows open/close
            (open_y, close_y)
        };

        extracted.instances.push(CandleInstance::with_bullish(
            x_pos, body_width, instance_open, high_y, low_y, instance_close, is_bullish,
        ));
    }

    // Debug: print first frame only
    #[cfg(debug_assertions)]
    if extracted.last_candle_count == 0 {
        println!("GPU Instancing Debug:");
        println!(
            "  Candles: {} (bull: {}, bear: {})",
            visible_candles.len(),
            bull_count,
            bear_count
        );
        println!("  Viewport: {:?}", viewport);
        println!("  Body width: {:.2}px | Style: {:?}", candle_width, candle_style);
        if let Some(first) = extracted.instances.first() {
            println!(
                "  First instance: x={}, w={}, o={}, h={}, l={}, c={}, bull={}",
                first.x_position,
                first.width,
                first.open,
                first.high,
                first.low,
                first.close,
                first.is_bullish
            );
        }
        println!(
            "  Colors: bull={:?}, bear={:?}, wick={:?}",
            extracted.bull_color, extracted.bear_color, extracted.wick_color
        );
    }

    // Debug: Check for duplicate x positions (would indicate 3x rendering bug)
    #[cfg(debug_assertions)]
    {
        use std::collections::HashMap;
        let mut x_counts: HashMap<i32, u32> = HashMap::new();
        for inst in &extracted.instances {
            let x_key = (inst.x_position * 100.0) as i32; // Round to 2 decimals
            *x_counts.entry(x_key).or_insert(0) += 1;
        }
        let duplicates: Vec<_> = x_counts.iter().filter(|(_, &count)| count > 1).collect();
        if !duplicates.is_empty() && duplicates.len() <= 10 {
            println!("[Extract] WARNING: Found {} duplicate x positions: {:?}",
                duplicates.len(), duplicates);
        }
    }

    // Update tracking state
    extracted.last_time_start = candle_data.candles[start].time;
    extracted.last_time_end = candle_data.candles[end - 1].time;
    extracted.last_candle_count = visible_candles.len();
    extracted.last_viewport_width = viewport_width;
    extracted.last_viewport_height = viewport_height;
    extracted.needs_redraw = true;
    extracted.instances_changed = true; // Signal that GPU buffer needs update
}

// ============================================================================
// PLUGIN
// ============================================================================

/// Plugin that adds GPU instanced rendering for candlestick charts
///
/// This plugin sets up the render pipeline, resources, and systems needed
/// for efficient GPU instanced rendering of candlesticks.
///
/// # Setup
/// - Registers `InstancingEnabled` in the main world
/// - Registers render resources in the render world
/// - Adds extraction, preparation, and rendering systems
/// - Creates the render pipeline in `finish()`
///
/// # Usage
/// ```no_run
/// use bevy::prelude::*;
/// use charts::rendering::CandlestickInstancedPlugin;
///
/// App::new()
///     .add_plugins(CandlestickInstancedPlugin)
///     .run();
/// ```
pub struct CandlestickInstancedPlugin;

impl Plugin for CandlestickInstancedPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(debug_assertions)]
        println!("CandlestickInstancedPlugin::build() called");

        // Register main world resource for toggling instancing
        app.init_resource::<InstancingEnabled>();

        // Get the render sub-app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            #[cfg(debug_assertions)]
            println!("WARNING: RenderApp not available!");
            return;
        };

        #[cfg(debug_assertions)]
        println!("RenderApp found, registering systems...");

        // Register render world resources
        render_app
            .init_resource::<ExtractedCandlesInstanced>()
            // Triple-buffered resources for GPU synchronization
            .init_resource::<CandleTripleBuffer>()
            // Add extract system to populate render world data
            .add_systems(ExtractSchedule, extract_candles_instanced)
            // Add prepare system to create/update GPU buffers
            .add_systems(Render, prepare_candles_instanced.in_set(RenderSystems::PrepareResources))
            // Add render graph node
            .add_render_graph_node::<ViewNodeRunner<CandlestickNode>>(
                Core2d,
                CandlestickInstancedLabel,
            )
            // Position after MainTransparentPass, before EndMainPass
            .add_render_graph_edges(
                Core2d,
                (
                    Node2d::MainTransparentPass,
                    CandlestickInstancedLabel,
                    Node2d::EndMainPass,
                ),
            );

        // Systems added:
        // - Extract system: extract_candles_instanced (ExtractSchedule) ✓
        // - Prepare system: prepare_candles_instanced (Render/Prepare) ✓
    }

    fn finish(&self, app: &mut App) {
        #[cfg(debug_assertions)]
        println!("CandlestickInstancedPlugin::finish() called");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            #[cfg(debug_assertions)]
            println!("WARNING: RenderApp not available in finish!");
            return;
        };

        #[cfg(debug_assertions)]
        println!("Creating pipeline...");

        let render_device = render_app.world().resource::<RenderDevice>();

        // Create bind group layout for ViewUniform
        let bind_group_layout = render_device.create_bind_group_layout(
            Some("candlestick_bind_group_layout"),
            &[
                // Binding 0: ViewUniform
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: ChartConfig
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Load shader
        let asset_server = render_app.world().resource::<AssetServer>();
        let shader = asset_server.load("shaders/candlestick_instanced.wgsl");

        // Get vertex buffer layout from CandleInstance
        let instance_layout = CandleInstance::vertex_buffer_layout_packed();

        // Create render pipeline descriptor
        let pipeline_cache = render_app.world().resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("candlestick_instanced_pipeline".into()),
            layout: vec![bind_group_layout.clone()],
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![instance_layout],
            },
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1, // Disabled MSAA - testing ghosting fix
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            zero_initialize_workgroup_memory: true,
            push_constant_ranges: vec![],
        });

        // Insert CandlePipeline resource
        render_app.insert_resource(CandlePipeline {
            pipeline_id,
            bind_group_layout,
        });

        #[cfg(debug_assertions)]
        println!("CandlePipeline inserted successfully");
    }
}
