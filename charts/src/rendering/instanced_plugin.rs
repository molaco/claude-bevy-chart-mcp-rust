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

use super::instancing::{CandleInstance, ViewUniform};
use crate::types::{Chart, ChartColors, PaneType};
use bevy::asset::AssetServer;
use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{
        NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
    },
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingType, BlendState,
        Buffer, BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, FragmentState, FrontFace, LoadOp, MultisampleState,
        Operations, PipelineCache, PolygonMode, PrimitiveState, PrimitiveTopology,
        RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages,
        StoreOp, TextureFormat, VertexState,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    view::ViewTarget,
    Extract, ExtractSchedule, Render, RenderApp, RenderSet,
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

/// Resource containing GPU buffers and bind groups for candlestick rendering
///
/// This resource manages the GPU-side data structures needed for instanced
/// rendering. Buffers are created lazily and resized as needed to accommodate
/// varying numbers of candlesticks.
///
/// # Buffer Management
/// - Instance buffer: Grows to fit all instances, never shrinks
/// - View uniform buffer: Fixed size (128 bytes for ViewUniform)
/// - Capacity tracking: Avoids unnecessary reallocations
///
/// # Bind Groups
/// The bind group references both the view uniform buffer and any textures/samplers
/// needed for rendering. It must be recreated when buffers are resized.
#[derive(Resource, Default)]
pub struct CandleRenderData {
    /// GPU buffer containing CandleInstance data
    pub instance_buffer: Option<Buffer>,

    /// Number of instances in the current buffer
    pub instance_count: u32,

    /// Allocated capacity of instance buffer (in instances, not bytes)
    pub buffer_capacity: usize,

    /// Bind group for view uniform and other resources
    pub bind_group: Option<BindGroup>,

    /// GPU buffer containing ViewUniform data
    pub view_uniform_buffer: Option<Buffer>,

    /// Allocated capacity of view uniform buffer (in ViewUniform structs)
    pub view_buffer_capacity: usize,

    /// Last viewport width used to create view buffer (for change detection)
    pub last_viewport_width: f32,

    /// Last viewport height used to create view buffer (for change detection)
    pub last_viewport_height: f32,
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
        // 1. Get render resources
        let render_data = world.resource::<CandleRenderData>();

        // Skip if no instances to render
        if render_data.instance_count == 0 {
            return Ok(());
        }

        // Get buffers and bind group (return early if not ready)
        let (Some(instance_buffer), Some(bind_group)) =
            (&render_data.instance_buffer, &render_data.bind_group)
        else {
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

        // 4. Set pipeline and bindings
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, *instance_buffer.slice(..));

        // 5. Draw: 12 vertices per candle, N instances
        const VERTICES_PER_CANDLE: u32 = 12;
        render_pass.draw(0..VERTICES_PER_CANDLE, 0..render_data.instance_count);

        // Debug: print once
        static DRAW_LOGGED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !DRAW_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            println!(
                "Render Node: Drawing {} instances ({} vertices)",
                render_data.instance_count,
                VERTICES_PER_CANDLE * render_data.instance_count
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
/// smart buffer reuse to minimize allocations and GPU uploads.
///
/// # Buffer Management Strategy
/// - Instance buffer: Only recreated when capacity is insufficient, reuses existing buffer otherwise
/// - View uniform buffer: Created once, updated only when viewport changes
/// - Bind groups: Created once with view uniform buffer
///
/// # Performance Optimizations
/// - Tracks buffer capacity to avoid unnecessary reallocations
/// - Only writes to GPU when data actually changed (instances_changed flag)
/// - Reuses buffers across frames for better performance
fn prepare_candles_instanced(
    mut render_data: ResMut<CandleRenderData>,
    extracted: Res<ExtractedCandlesInstanced>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Option<Res<CandlePipeline>>,
) {
    // Debug: always print once to confirm system runs
    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.load(std::sync::atomic::Ordering::Relaxed) {
        println!(
            "Prepare Debug: system entered, pipeline exists: {}",
            pipeline.is_some()
        );
    }

    let Some(pipeline) = pipeline else {
        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            println!("Prepare Debug: NO PIPELINE - skipping");
        }
        return;
    };

    // Early return if no data
    if extracted.instances.is_empty() {
        render_data.instance_count = 0;
        return;
    }

    if !LOGGED.load(std::sync::atomic::Ordering::Relaxed) {
        println!(
            "Prepare Debug: {} instances, buffer_capacity={}",
            extracted.instances.len(),
            render_data.buffer_capacity
        );
    }

    // A. Create/update instance buffer
    let instance_data: &[u8] = bytemuck::cast_slice(&extracted.instances);
    let required_size = instance_data.len();

    // Only recreate buffer if capacity insufficient
    if render_data.buffer_capacity < required_size {
        render_data.instance_buffer = Some(render_device.create_buffer_with_data(
            &BufferInitDescriptor {
                label: Some("candlestick_instance_buffer"),
                contents: instance_data,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            },
        ));
        render_data.buffer_capacity = required_size;
    } else if let Some(buffer) = &render_data.instance_buffer {
        // Reuse existing buffer, just write new data
        render_queue.write_buffer(buffer, 0, instance_data);
    }

    // B. Build view uniform
    let view_uniform = ViewUniform {
        view_proj: build_orthographic_matrix(extracted.viewport_width, extracted.viewport_height),
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

    // C. Create or update uniform buffer
    if render_data.view_uniform_buffer.is_none() {
        render_data.view_uniform_buffer = Some(render_device.create_buffer_with_data(
            &BufferInitDescriptor {
                label: Some("candlestick_view_uniform_buffer"),
                contents: uniform_data,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            },
        ));
    } else if let Some(buffer) = &render_data.view_uniform_buffer {
        render_queue.write_buffer(buffer, 0, uniform_data);
    }

    // D. Create bind group (if buffers exist)
    if let Some(view_buffer) = &render_data.view_uniform_buffer {
        render_data.bind_group = Some(render_device.create_bind_group(
            Some("candlestick_bind_group"),
            &pipeline.bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        ));
    }

    // E. Update instance count
    render_data.instance_count = extracted.instances.len() as u32;

    // Debug: confirm buffer creation
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        println!(
            "Prepare Complete: instance_count={}, has_buffer={}, has_bind_group={}",
            render_data.instance_count,
            render_data.instance_buffer.is_some(),
            render_data.bind_group.is_some()
        );
    }
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
    chart: Extract<Res<Chart>>,
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
    if chart.candles.is_empty() {
        return;
    }

    // Get window size for viewport tracking
    let (viewport_width, viewport_height) = if let Ok(window) = windows.single() {
        (window.width(), window.height())
    } else {
        (extracted.viewport_width, extracted.viewport_height)
    };

    // Find the Price pane
    let price_pane = match chart
        .panes
        .iter()
        .find(|p| matches!(p.pane_type, PaneType::Price))
    {
        Some(pane) => pane,
        None => return,
    };

    // Get visible candle range
    let start = chart.visible_candle_start;
    let count = chart.visible_candle_count;
    let end = (start + count).min(chart.candles.len());

    if start >= end {
        return;
    }

    let visible_candles = &chart.candles[start..end];

    // Use ChartSpace's price range (same as old rendering system)
    let price_min = price_pane.space.visible_price_min;
    let price_max = price_pane.space.visible_price_max;
    let price_range = price_max - price_min;

    if price_range <= 0.0 {
        return;
    }

    // Check if state changed - skip expensive rebuild if nothing changed
    let state_unchanged = extracted.last_time_start == chart.candles[start].time
        && extracted.last_time_end == chart.candles[end - 1].time
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
    let visible_count = chart.visible_candle_count;
    let candle_width = viewport.width() / visible_count.max(1) as f32;
    let body_width = candle_width * 0.7; // 70% for body (same as old system)

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

        extracted.instances.push(CandleInstance::with_bullish(
            x_pos, body_width, open_y, high_y, low_y, close_y, is_bullish,
        ));
    }

    // Debug: print first frame only
    if extracted.last_candle_count == 0 {
        println!("GPU Instancing Debug:");
        println!(
            "  Candles: {} (bull: {}, bear: {})",
            visible_candles.len(),
            bull_count,
            bear_count
        );
        println!("  Viewport: {:?}", viewport);
        println!("  Body width: {}", body_width);
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

    // Update tracking state
    extracted.last_time_start = chart.candles[start].time;
    extracted.last_time_end = chart.candles[end - 1].time;
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
        println!("CandlestickInstancedPlugin::build() called");

        // Register main world resource for toggling instancing
        app.init_resource::<InstancingEnabled>();

        // Get the render sub-app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            println!("WARNING: RenderApp not available!");
            return;
        };

        println!("RenderApp found, registering systems...");

        // Register render world resources
        render_app
            .init_resource::<ExtractedCandlesInstanced>()
            .init_resource::<CandleRenderData>()
            // Add extract system to populate render world data
            .add_systems(ExtractSchedule, extract_candles_instanced)
            // Add prepare system to create/update GPU buffers
            .add_systems(Render, prepare_candles_instanced.in_set(RenderSet::Prepare))
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
        println!("CandlestickInstancedPlugin::finish() called");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            println!("WARNING: RenderApp not available in finish!");
            return;
        };

        println!("Creating pipeline...");

        let render_device = render_app.world().resource::<RenderDevice>();

        // Create bind group layout for ViewUniform
        let bind_group_layout = render_device.create_bind_group_layout(
            Some("candlestick_bind_group_layout"),
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
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
                count: 4, // Match the MSAA sample count used by Bevy's default render pass
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

        println!("CandlePipeline inserted successfully");
    }
}
