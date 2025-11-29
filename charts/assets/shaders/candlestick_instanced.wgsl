// GPU Instanced Candlestick Rendering Shader
// Each instance represents one candlestick with wick and body

struct ViewUniform {
    view_proj: mat4x4<f32>,
    viewport: vec4<f32>,
    bull_color: vec4<f32>,
    bear_color: vec4<f32>,
    wick_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: ViewUniform;

struct VertexInput {
    @location(0) data0: vec4<f32>,  // (x_position, width, open, high)
    @location(1) data1: vec4<f32>,  // (low, close, is_bullish, _padding)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

// Helper function to get quad X coordinate based on vertex index
fn get_quad_x(index: u32) -> f32 {
    switch index {
        case 0u: { return -0.5; }
        case 1u: { return 0.5; }
        case 2u: { return 0.5; }
        case 3u: { return -0.5; }
        case 4u: { return 0.5; }
        default: { return -0.5; }  // case 5u
    }
}

// Helper function to get quad Y coordinate based on vertex index
fn get_quad_y(index: u32) -> f32 {
    switch index {
        case 0u: { return 0.0; }
        case 1u: { return 0.0; }
        case 2u: { return 1.0; }
        case 3u: { return 0.0; }
        case 4u: { return 1.0; }
        default: { return 1.0; }  // case 5u
    }
}

@vertex
fn vertex(
    input: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Unpack instance data
    let x_position = input.data0.x;
    let width = input.data0.y;
    let open = input.data0.z;
    let high = input.data0.w;
    let low = input.data1.x;
    let close = input.data1.y;
    let is_bullish = input.data1.z;

    // Determine which part we're rendering (wick or body)
    let local_vertex_index = vertex_index % 6u;
    let is_wick = vertex_index < 6u;

    var position: vec2<f32>;
    var color: vec4<f32>;
    var z_depth: f32;

    if (is_wick) {
        // Render wick (thin vertical line from low to high)
        let wick_width = max(width * 0.1, 1.0);
        let wick_height = high - low;

        let local_x = get_quad_x(local_vertex_index) * wick_width;
        let local_y = get_quad_y(local_vertex_index) * wick_height;

        position = vec2<f32>(
            x_position + local_x,
            low + local_y
        );

        color = view.wick_color;
        z_depth = 0.0;  // Behind body
    } else {
        // Render body (rectangle from open to close)
        let body_bottom = min(open, close);
        let body_top = max(open, close);
        let body_height = body_top - body_bottom;

        let local_x = get_quad_x(local_vertex_index) * width;
        let local_y = get_quad_y(local_vertex_index) * body_height;

        position = vec2<f32>(
            x_position + local_x,
            body_bottom + local_y
        );

        // Choose color based on bullish/bearish
        if (is_bullish > 0.5) {
            color = view.bull_color;
        } else {
            color = view.bear_color;
        }

        z_depth = 0.1;  // In front of wick
    }

    // Transform to clip space
    let world_position = vec4<f32>(position.x, position.y, z_depth, 1.0);
    out.clip_position = view.view_proj * world_position;
    out.color = color;

    return out;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
