//! GPU instanced rendering plugin for volume bars
//!
//! This module provides a Bevy plugin that implements GPU instanced rendering
//! for volume bars, similar to the candlestick instanced plugin.

use super::instancing::{ViewUniform, VolumeInstance};
use super::triple_buffer::VolumeTripleBuffer;
use crate::types::{CandleData, ChartColors, PaneId, PaneManager, ViewportState};
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

use super::instanced_plugin::CandlestickInstancedLabel;

/// Resource containing extracted volume instance data
#[derive(Resource, Default)]
pub struct ExtractedVolumesInstanced {
    /// Instance data for all visible volume bars
    pub instances: Vec<VolumeInstance>,

    /// True if instance data has changed since last frame
    pub instances_changed: bool,

    /// Current viewport width in pixels
    pub viewport_width: f32,

    /// Current viewport height in pixels
    pub viewport_height: f32,

    // State tracking for change detection
    pub last_time_start: i64,
    pub last_time_end: i64,
    pub last_candle_count: usize,
    pub last_viewport_width: f32,
    pub last_viewport_height: f32,

    // Theme colors (linear RGBA)
    pub bull_color: [f32; 4],
    pub bear_color: [f32; 4],
}

/// Resource containing the cached render pipeline for volumes
#[derive(Resource)]
pub struct VolumePipeline {
    pub pipeline_id: CachedRenderPipelineId,
    pub bind_group_layout: BindGroupLayout,
}

/// Render graph label for volume instanced rendering node
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct VolumeInstancedLabel;

/// Render graph node for GPU instanced volume rendering
#[derive(Default)]
pub struct VolumeNode;

impl ViewNode for VolumeNode {
    type ViewQuery = &'static ViewTarget;

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        view_target: QueryItem<'w, 'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Get triple-buffered resources directly
        let triple_buffer = world.resource::<VolumeTripleBuffer>();

        if triple_buffer.resources.instance_count == 0 {
            return Ok(());
        }

        // Get the slot that was written to in prepare phase
        // Since prepare calls complete_frame() which advances the counter,
        // we look back 1 frame to find the slot containing current frame's data
        let render_slot_index = triple_buffer.resources.index_frames_ago(1);
        let slot = &triple_buffer.resources.slots[render_slot_index];

        let (Some(instance_buffer), Some(bind_group)) =
            (&slot.instance_buffer, &slot.bind_group)
        else {
            return Ok(());
        };

        let Some(pipeline_resource) = world.get_resource::<VolumePipeline>() else {
            return Ok(());
        };

        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_resource.pipeline_id)
        else {
            return Ok(());
        };

        let color_attachment = view_target.get_color_attachment();

        let mut render_pass =
            render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("volume_instanced_pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: color_attachment.view,
                        resolve_target: color_attachment.resolve_target,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, *instance_buffer.slice(..));

        // 6 vertices per volume bar (2 triangles for quad)
        const VERTICES_PER_BAR: u32 = 6;
        let instance_count = triple_buffer.resources.instance_count;
        render_pass.draw(0..VERTICES_PER_BAR, 0..instance_count);

        Ok(())
    }
}

/// Build orthographic projection matrix (same as candlestick)
fn build_orthographic_matrix(width: f32, height: f32) -> [[f32; 4]; 4] {
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

/// Prepare GPU buffers for volume rendering with triple buffering
///
/// Uses 3 rotating instance buffers to eliminate CPU/GPU synchronization stalls.
fn prepare_volumes_instanced(
    mut triple_buffer: ResMut<VolumeTripleBuffer>,
    extracted: Res<ExtractedVolumesInstanced>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Option<Res<VolumePipeline>>,
) {
    let Some(pipeline) = pipeline else {
        triple_buffer.resources.skip_frame();
        return;
    };

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
            "[Volume] Pre-allocating 3 instance buffers: {} bytes data, {} bytes capacity ({}x headroom)",
            required_size,
            initial_capacity,
            initial_capacity as f32 / required_size as f32
        );

        // Create a zero-filled buffer at full capacity, then write actual data
        let mut padded_data = vec![0u8; initial_capacity];
        padded_data[..required_size].copy_from_slice(instance_data);

        for i in 0..super::triple_buffer::BUFFER_COUNT {
            let slot = &mut triple_buffer.resources.slots[i];
            if slot.instance_buffer.is_none() {
                let label = match i {
                    0 => "volume_instance_buffer_slot0",
                    1 => "volume_instance_buffer_slot1",
                    _ => "volume_instance_buffer_slot2",
                };
                slot.instance_buffer = Some(render_device.create_buffer_with_data(
                    &BufferInitDescriptor {
                        label: Some(label),
                        contents: &padded_data,
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    },
                ));
                slot.buffer_capacity = initial_capacity;
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

    // Step 1: Begin frame - acquire current slot
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
            "[Volume] Frame {}: Write slot {}, GPU reads slot {}, InFlight slot {} | {} instances | fence_wait={}µs",
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
                "[Volume] Reallocating slot {}: {} -> {} bytes (required: {})",
                slot_index, old_capacity, new_capacity, required_size
            );

            // Create padded buffer at full capacity
            let mut padded_data = vec![0u8; new_capacity];
            padded_data[..required_size].copy_from_slice(instance_data);

            let label = match slot_index {
                0 => "volume_instance_buffer_slot0",
                1 => "volume_instance_buffer_slot1",
                _ => "volume_instance_buffer_slot2",
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
        } else if extracted.instances_changed {
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
        wick_color: [0.0, 0.0, 0.0, 0.0], // Not used for volume
    };

    let uniform_data: &[u8] = bytemuck::bytes_of(&view_uniform);
    let uniform_size = uniform_data.len();

    // Write to current slot's uniform buffer (each slot has its own)
    {
        let slot = &mut triple_buffer.resources.slots[slot_index];

        if slot.needs_uniform_reallocation(uniform_size) {
            let label = match slot_index {
                0 => "volume_view_uniform_slot0",
                1 => "volume_view_uniform_slot1",
                _ => "volume_view_uniform_slot2",
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
        [0.0, 0.0, 0.0, 0.0], // Volume doesn't use wick color
    );

    // ========================================================================
    // BIND GROUP (Per-slot, references slot's own uniform buffer)
    // ========================================================================

    let needs_bind_group = {
        let slot = &triple_buffer.resources.slots[slot_index];
        (slot.bind_group.is_none() || !slot.bind_group_valid)
            && slot.instance_buffer.is_some()
            && slot.view_uniform_buffer.is_some()
    };

    if needs_bind_group {
        let slot = &triple_buffer.resources.slots[slot_index];
        let view_buffer = slot.view_uniform_buffer.as_ref().unwrap();

        let label = match slot_index {
            0 => "volume_bind_group_slot0",
            1 => "volume_bind_group_slot1",
            _ => "volume_bind_group_slot2",
        };

        let bind_group = render_device.create_bind_group(
            Some(label),
            &pipeline.bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        );

        let slot = &mut triple_buffer.resources.slots[slot_index];
        slot.bind_group = Some(bind_group);
        slot.bind_group_valid = true;
    }

    // Update instance count
    triple_buffer.resources.instance_count = extracted.instances.len() as u32;

    // Complete debug frame tracking
    triple_buffer.resources.debug_end_frame();

    // Step 3: Complete frame - mark slot as submitted, advance frame counter
    triple_buffer.resources.complete_frame();
}

/// Convert sRGB color to linear RGBA array
fn color_to_array(color: &Color) -> [f32; 4] {
    let linear = color.to_linear();
    [linear.red, linear.green, linear.blue, linear.alpha]
}

/// Extract volume data from main world to render world
fn extract_volumes_instanced(
    mut extracted: ResMut<ExtractedVolumesInstanced>,
    candle_data: Extract<Res<CandleData>>,
    viewport_state: Extract<Res<ViewportState>>,
    pane_manager: Extract<Res<PaneManager>>,
    chart_colors: Extract<Res<ChartColors>>,
    windows: Extract<Query<&Window>>,
) {
    extracted.instances_changed = false;

    // Extract colors
    extracted.bull_color = color_to_array(&chart_colors.bull_candle);
    extracted.bear_color = color_to_array(&chart_colors.bear_candle);

    if candle_data.candles.is_empty() {
        extracted.instances.clear();
        return;
    }

    // Get window size
    let (viewport_width, viewport_height) = if let Ok(window) = windows.single() {
        (window.width(), window.height())
    } else {
        (extracted.viewport_width, extracted.viewport_height)
    };

    // Find Volume pane
    let volume_pane = match pane_manager.find_pane(PaneId::Volume) {
        Some(pane) => pane,
        None => {
            extracted.instances.clear();
            return;
        }
    };

    let start = viewport_state.visible_candle_start;
    let count = viewport_state.visible_candle_count;
    let end = (start + count).min(candle_data.candles.len());

    if start >= end {
        extracted.instances.clear();
        return;
    }

    let visible_candles = &candle_data.candles[start..end];

    // Check if state changed
    let state_unchanged = extracted.last_time_start == candle_data.candles[start].time
        && extracted.last_time_end == candle_data.candles[end - 1].time
        && extracted.last_candle_count == visible_candles.len()
        && (extracted.last_viewport_width - viewport_width).abs() < 1.0
        && (extracted.last_viewport_height - viewport_height).abs() < 1.0
        && !extracted.instances.is_empty();

    if state_unchanged {
        return;
    }

    // State changed - rebuild
    extracted.instances.clear();
    extracted.viewport_width = viewport_width;
    extracted.viewport_height = viewport_height;

    let viewport = &volume_pane.space.viewport;
    let price_min = volume_pane.space.visible_price_min;
    let price_max = volume_pane.space.visible_price_max;
    let price_range = price_max - price_min;

    if price_range <= 0.0 {
        return;
    }

    let visible_count = viewport_state.visible_candle_count;
    let candle_width = viewport.width() / visible_count.max(1) as f32;
    let bar_width = candle_width * crate::config::VOLUME_BAR_WIDTH_RATIO;

    extracted.instances.reserve(visible_candles.len());

    for (idx, candle) in visible_candles.iter().enumerate() {
        let candle_index = start + idx;

        // Calculate x position
        let candle_offset = candle_index.saturating_sub(start);
        let x_percent = candle_offset as f32 / visible_count as f32;
        let x_pos = viewport.min.x + x_percent * viewport.width();

        // Calculate Y positions in world space
        let y_bottom = viewport.min.y;
        let volume_percent = (candle.volume as f32 - price_min) / price_range;
        let bar_height = (volume_percent * viewport.height()).max(1.0);
        let y_top = y_bottom + bar_height;

        let is_bullish = candle.close >= candle.open;

        extracted.instances.push(VolumeInstance::new(
            x_pos, bar_width, y_bottom, y_top, is_bullish,
        ));
    }

    // Update tracking
    extracted.last_time_start = candle_data.candles[start].time;
    extracted.last_time_end = candle_data.candles[end - 1].time;
    extracted.last_candle_count = visible_candles.len();
    extracted.last_viewport_width = viewport_width;
    extracted.last_viewport_height = viewport_height;
    extracted.instances_changed = true;
}

/// Plugin for GPU instanced volume bar rendering
pub struct VolumeInstancedPlugin;

impl Plugin for VolumeInstancedPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<ExtractedVolumesInstanced>()
            // Triple-buffered resources for GPU synchronization
            .init_resource::<VolumeTripleBuffer>()
            .add_systems(ExtractSchedule, extract_volumes_instanced)
            .add_systems(
                Render,
                prepare_volumes_instanced.in_set(RenderSystems::PrepareResources),
            )
            .add_render_graph_node::<ViewNodeRunner<VolumeNode>>(Core2d, VolumeInstancedLabel)
            // Volume renders after candlesticks
            .add_render_graph_edges(
                Core2d,
                (
                    CandlestickInstancedLabel,
                    VolumeInstancedLabel,
                    Node2d::EndMainPass,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        let render_device = render_app.world().resource::<RenderDevice>();

        let bind_group_layout = render_device.create_bind_group_layout(
            Some("volume_bind_group_layout"),
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

        let asset_server = render_app.world().resource::<AssetServer>();
        let shader = asset_server.load("shaders/volume_instanced.wgsl");

        let instance_layout = VolumeInstance::vertex_buffer_layout();

        let pipeline_cache = render_app.world().resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("volume_instanced_pipeline".into()),
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
                count: 4,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            zero_initialize_workgroup_memory: true,
            push_constant_ranges: vec![],
        });

        render_app.insert_resource(VolumePipeline {
            pipeline_id,
            bind_group_layout,
        });
    }
}
