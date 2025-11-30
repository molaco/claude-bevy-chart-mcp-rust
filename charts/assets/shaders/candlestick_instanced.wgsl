// GPU Instanced Candlestick Rendering Shader
// Each instance represents one candlestick with wick and body
// Renders 12 vertices per instance: 6 for wick (0-5), 6 for body (6-11)

struct ViewUniform {
    view_proj: mat4x4<f32>,
    viewport: vec4<f32>,
    bull_color: vec4<f32>,
    bear_color: vec4<f32>,
    wick_color: vec4<f32>,
}

struct ChartConfig {
    wick_width_ratio: f32,    // Wick width as fraction of body width (default: 0.15)
    min_wick_width: f32,      // Minimum wick width in pixels (default: 2.0)
    min_element_height: f32,  // Minimum element height in pixels (default: 1.0)
    _padding: f32,
}

@group(0) @binding(0) var<uniform> view: ViewUniform;
@group(0) @binding(1) var<uniform> config: ChartConfig;

struct VertexInput {
    @location(0) data0: vec4<f32>,  // (x_position, width, open, high)
    @location(1) data1: vec4<f32>,  // (low, close, is_bullish, _padding)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

// Helper function to get quad X coordinate based on vertex index
// Returns values for: bottom-left, bottom-right, top-right, bottom-left, top-right, top-left
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

// Helper function to get quad Y coordinate based on vertex index
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
    let open = input.data0.z;
    let high = input.data0.w;
    let low = input.data1.x;
    let close = input.data1.y;
    let is_bullish = input.data1.z;

    // Each candle uses 12 vertices: 0-5 for wick (drawn first), 6-11 for body (drawn on top)
    let is_body = vertex_index >= 6u;
    let local_vert = vertex_index % 6u;

    var pos: vec2<f32>;
    var color: vec4<f32>;

    // Calculate wick width using config values
    let wick_width = max(width * config.wick_width_ratio, config.min_wick_width);

    // Get normalized quad position using helper functions
    let quad_x = get_quad_x(local_vert);
    let quad_y = get_quad_y(local_vert);

    if is_body {
        // Body: wider rectangle (drawn second, on top of wick)
        let body_width = max(width, wick_width);
        let body_bottom = min(open, close);
        let body_top = max(open, close);
        let body_height = max(body_top - body_bottom, config.min_element_height);

        pos.x = x_position + quad_x * body_width;
        pos.y = body_bottom + quad_y * body_height;

        // Color based on bullish/bearish
        if is_bullish > 0.5 {
            color = view.bull_color;
        } else {
            color = view.bear_color;
        }
    } else {
        // Wick: thin vertical line (drawn first, behind body)
        let wick_height = high - low;

        pos.x = x_position + quad_x * wick_width;
        pos.y = low + quad_y * max(wick_height, config.min_element_height);
        color = view.wick_color;
    }

    // Transform from world coordinates to clip space
    var out: VertexOutput;
    out.clip_position = view.view_proj * vec4<f32>(pos, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
