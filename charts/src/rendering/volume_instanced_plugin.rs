//! GPU instanced rendering plugin for volume bars
//!
//! This module provides a Bevy plugin that implements GPU instanced rendering
//! for volume bars, similar to the candlestick instanced plugin.

use super::instancing::{ViewUniform, VolumeInstance};
use crate::types::{CandleData, ChartColors, PaneId, PaneManager, ViewportState};
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

/// Resource containing GPU buffers and bind groups for volume rendering
#[derive(Resource, Default)]
pub struct VolumeRenderData {
    pub instance_buffer: Option<Buffer>,
    pub instance_count: u32,
    pub buffer_capacity: usize,
    pub bind_group: Option<BindGroup>,
    pub view_uniform_buffer: Option<Buffer>,

    // Change detection
    pub last_viewport_width: f32,
    pub last_viewport_height: f32,
    pub last_bull_color: [f32; 4],
    pub last_bear_color: [f32; 4],
    pub bind_group_valid: bool,
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
        let render_data = world.resource::<VolumeRenderData>();

        if render_data.instance_count == 0 {
            return Ok(());
        }

        let (Some(instance_buffer), Some(bind_group)) =
            (&render_data.instance_buffer, &render_data.bind_group)
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
        render_pass.draw(0..VERTICES_PER_BAR, 0..render_data.instance_count);

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

/// Prepare GPU buffers for volume rendering
fn prepare_volumes_instanced(
    mut render_data: ResMut<VolumeRenderData>,
    extracted: Res<ExtractedVolumesInstanced>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Option<Res<VolumePipeline>>,
) {
    let Some(pipeline) = pipeline else {
        return;
    };

    if extracted.instances.is_empty() {
        render_data.instance_count = 0;
        return;
    }

    // A. Create/update instance buffer
    let instance_data: &[u8] = bytemuck::cast_slice(&extracted.instances);
    let required_size = instance_data.len();

    if render_data.buffer_capacity < required_size {
        render_data.instance_buffer = Some(render_device.create_buffer_with_data(
            &BufferInitDescriptor {
                label: Some("volume_instance_buffer"),
                contents: instance_data,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            },
        ));
        render_data.buffer_capacity = required_size;
    } else if extracted.instances_changed {
        if let Some(buffer) = &render_data.instance_buffer {
            render_queue.write_buffer(buffer, 0, instance_data);
        }
    }

    // B. Check if view uniform changed
    let viewport_changed =
        (render_data.last_viewport_width - extracted.viewport_width).abs() > 0.1
            || (render_data.last_viewport_height - extracted.viewport_height).abs() > 0.1;

    let colors_changed = render_data.last_bull_color != extracted.bull_color
        || render_data.last_bear_color != extracted.bear_color;

    let view_uniform_changed =
        viewport_changed || colors_changed || render_data.view_uniform_buffer.is_none();

    // C. Build and update view uniform only when changed
    if view_uniform_changed {
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

        if render_data.view_uniform_buffer.is_none() {
            render_data.view_uniform_buffer = Some(render_device.create_buffer_with_data(
                &BufferInitDescriptor {
                    label: Some("volume_view_uniform_buffer"),
                    contents: uniform_data,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                },
            ));
            render_data.bind_group_valid = false;
        } else if let Some(buffer) = &render_data.view_uniform_buffer {
            render_queue.write_buffer(buffer, 0, uniform_data);
        }

        render_data.last_viewport_width = extracted.viewport_width;
        render_data.last_viewport_height = extracted.viewport_height;
        render_data.last_bull_color = extracted.bull_color;
        render_data.last_bear_color = extracted.bear_color;
    }

    // D. Create bind group only when needed
    if let Some(view_buffer) = &render_data.view_uniform_buffer {
        let needs_bind_group =
            render_data.bind_group.is_none() || !render_data.bind_group_valid;

        if needs_bind_group {
            render_data.bind_group = Some(render_device.create_bind_group(
                Some("volume_bind_group"),
                &pipeline.bind_group_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: view_buffer.as_entire_binding(),
                }],
            ));
            render_data.bind_group_valid = true;
        }
    }

    render_data.instance_count = extracted.instances.len() as u32;
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
            .init_resource::<VolumeRenderData>()
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
