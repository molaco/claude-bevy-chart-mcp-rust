// GPU Instanced Volume Bar Rendering Shader
// Each instance represents one volume bar
// Renders 6 vertices per instance: 2 triangles for a quad

struct ViewUniform {
    view_proj: mat4x4<f32>,
    viewport: vec4<f32>,
    bull_color: vec4<f32>,
    bear_color: vec4<f32>,
    wick_color: vec4<f32>,  // Not used for volume but keeps uniform consistent
}

@group(0) @binding(0) var<uniform> view: ViewUniform;

struct VertexInput {
    @location(0) data0: vec4<f32>,  // (x_position, width, y_bottom, y_top)
    @location(1) data1: vec4<f32>,  // (is_bullish, _padding, _padding, _padding)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

// Helper function to get quad X coordinate based on vertex index
fn get_quad_x(index: u32) -> f32 {
    switch index {
        case 0u: { return -0.5; }  // bottom-left
        case 1u: { return 0.5; }   // bottom-right
        case 2u: { return 0.5; }   // top-right
        case 3u: { return -0.5; }  // bottom-left (triangle 2)
        case 4u: { return 0.5; }   // top-right
        default: { return -0.5; }  // case 5u: top-left
    }
}

// Helper function to get quad Y coordinate based on vertex index (0 = bottom, 1 = top)
fn get_quad_y(index: u32) -> f32 {
    switch index {
        case 0u: { return 0.0; }  // bottom
        case 1u: { return 0.0; }  // bottom
        case 2u: { return 1.0; }  // top
        case 3u: { return 0.0; }  // bottom
        case 4u: { return 1.0; }  // top
        default: { return 1.0; }  // case 5u: top
    }
}

@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    input: VertexInput,
) -> VertexOutput {
    // Unpack instance data
    let x_position = input.data0.x;
    let width = input.data0.y;
    let y_bottom = input.data0.z;
    let y_top = input.data0.w;
    let is_bullish = input.data1.x;

    // Get normalized quad position
    let quad_x = get_quad_x(vertex_index);
    let quad_y = get_quad_y(vertex_index);

    // Calculate world position
    var pos: vec2<f32>;
    pos.x = x_position + quad_x * width;
    // Interpolate between y_bottom and y_top based on quad_y
    pos.y = y_bottom + quad_y * (y_top - y_bottom);

    // Color based on bullish/bearish
    var color: vec4<f32>;
    if is_bullish > 0.5 {
        color = view.bull_color;
    } else {
        color = view.bear_color;
    }

    var out: VertexOutput;
    out.clip_position = view.view_proj * vec4<f32>(pos, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
