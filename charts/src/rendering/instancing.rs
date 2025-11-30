//! GPU-compatible instancing data structures for candlestick rendering
//!
//! This module defines the CandleInstance struct that is specifically designed
//! for GPU instanced rendering in Bevy. The struct layout is carefully crafted
//! to meet GPU memory alignment requirements.

use bytemuck::{Pod, Zeroable};
use bevy::mesh::VertexBufferLayout;
use bevy::render::render_resource::{VertexStepMode, VertexAttribute, VertexFormat};

/// GPU-compatible instance data for a single candlestick
///
/// This struct is designed for instanced rendering on the GPU with strict
/// memory layout requirements:
/// - Total size: 32 bytes (8 x f32 fields)
/// - Alignment: 16 bytes (required for GPU uniform buffers)
/// - Layout: C-compatible using #[repr(C)]
///
/// # Memory Layout
/// Each field is 4 bytes (f32):
/// - x_position: horizontal position in world space
/// - width: candlestick width in pixels
/// - open: opening price
/// - high: highest price
/// - low: lowest price
/// - close: closing price
/// - is_bullish: 1.0 for bullish (green), 0.0 for bearish (red)
/// - _padding: ensures 32-byte total size
///
/// # Safety
/// The struct implements Pod and Zeroable which allows safe casting to/from
/// byte arrays for GPU buffer uploads. This is safe because:
/// - All fields are f32 (valid for any bit pattern)
/// - #[repr(C)] ensures predictable memory layout
/// - Size and alignment are verified at compile time
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CandleInstance {
    /// Horizontal position in world space coordinates
    pub x_position: f32,

    /// Width of the candlestick body in pixels
    pub width: f32,

    /// Opening price value
    pub open: f32,

    /// Highest price value (top of wick)
    pub high: f32,

    /// Lowest price value (bottom of wick)
    pub low: f32,

    /// Closing price value
    pub close: f32,

    /// Bullish indicator: 1.0 = bullish (green), 0.0 = bearish (red)
    pub is_bullish: f32,

    /// Padding to ensure 32-byte total size (required for GPU alignment)
    pub _padding: f32,
}

// Compile-time assertions to verify struct layout requirements
const _: () = {
    // Verify total size is exactly 32 bytes
    assert!(
        std::mem::size_of::<CandleInstance>() == 32,
        "CandleInstance must be exactly 32 bytes"
    );

    // Verify alignment is 16 bytes (required for GPU uniform buffers)
    assert!(
        std::mem::align_of::<CandleInstance>() == 4,
        "CandleInstance must have 4-byte alignment (natural f32 alignment)"
    );
};

impl CandleInstance {
    /// Create a new CandleInstance with all parameters
    ///
    /// # Arguments
    /// * `x_position` - Horizontal position in world space
    /// * `width` - Candlestick width in pixels
    /// * `open` - Opening price
    /// * `high` - Highest price (top of wick)
    /// * `low` - Lowest price (bottom of wick)
    /// * `close` - Closing price
    ///
    /// # Returns
    /// A new CandleInstance with is_bullish automatically determined by
    /// comparing close and open prices.
    ///
    /// # Example
    /// ```
    /// use charts::rendering::CandleInstance;
    /// let candle = CandleInstance::new(100.0, 8.0, 50.0, 52.0, 49.0, 51.0);
    /// assert_eq!(candle.is_bullish, 1.0); // close > open
    /// ```
    pub fn new(
        x_position: f32,
        width: f32,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
    ) -> Self {
        let is_bullish = if close >= open { 1.0 } else { 0.0 };

        Self {
            x_position,
            width,
            open,
            high,
            low,
            close,
            is_bullish,
            _padding: 0.0,
        }
    }

    /// Create a new CandleInstance with explicit bullish flag
    ///
    /// Use this constructor when you need to override the automatic bullish
    /// determination (e.g., for special rendering modes or custom indicators).
    ///
    /// # Arguments
    /// * `x_position` - Horizontal position in world space
    /// * `width` - Candlestick width in pixels
    /// * `open` - Opening price
    /// * `high` - Highest price (top of wick)
    /// * `low` - Lowest price (bottom of wick)
    /// * `close` - Closing price
    /// * `is_bullish` - True for bullish (green), false for bearish (red)
    ///
    /// # Example
    /// ```
    /// use charts::rendering::CandleInstance;
    /// let candle = CandleInstance::with_bullish(100.0, 8.0, 50.0, 52.0, 49.0, 51.0, false);
    /// assert_eq!(candle.is_bullish, 0.0); // forced bearish
    /// ```
    pub fn with_bullish(
        x_position: f32,
        width: f32,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        is_bullish: bool,
    ) -> Self {
        Self {
            x_position,
            width,
            open,
            high,
            low,
            close,
            is_bullish: if is_bullish { 1.0 } else { 0.0 },
            _padding: 0.0,
        }
    }

    /// Returns the vertex buffer layout for packed Float32x4 attributes
    ///
    /// This layout uses two vec4 attributes for optimal GPU performance:
    /// - Location 0 (offset 0): (x_position, width, open, high)
    /// - Location 1 (offset 16): (low, close, is_bullish, _padding)
    ///
    /// This packed format reduces the number of vertex attributes and aligns
    /// with GPU vector units for better performance.
    ///
    /// # Memory Layout
    /// - array_stride: 32 bytes (size of CandleInstance)
    /// - step_mode: Instance (one instance per candlestick)
    /// - Total attributes: 2 (both Float32x4)
    pub fn vertex_buffer_layout_packed() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<CandleInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                // Location 0: First vec4 (x_position, width, open, high)
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                // Location 1: Second vec4 (low, close, is_bullish, _padding)
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 1,
                },
            ],
        }
    }

    /// Returns the vertex buffer layout with unpacked Float32 attributes
    ///
    /// This layout uses individual float attributes for each field, which
    /// may be more convenient for shader development but less efficient than
    /// the packed layout.
    ///
    /// # Attributes
    /// - Location 0: x_position
    /// - Location 1: width
    /// - Location 2: open
    /// - Location 3: high
    /// - Location 4: low
    /// - Location 5: close
    /// - Location 6: is_bullish
    /// - (padding is not exposed to shaders)
    pub fn vertex_buffer_layout_unpacked() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<CandleInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 0,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 4,
                    shader_location: 1,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 8,
                    shader_location: 2,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 12,
                    shader_location: 3,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 16,
                    shader_location: 4,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 20,
                    shader_location: 5,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 24,
                    shader_location: 6,
                },
            ],
        }
    }
}

/// GPU-compatible uniform data for view and rendering parameters
///
/// This struct contains the projection matrix, viewport information, and color
/// settings that are shared across all candlestick instances in a single draw call.
///
/// # Memory Layout
/// Total size: 128 bytes, organized as:
/// - view_proj: 64 bytes (4x4 matrix for orthographic projection)
/// - viewport: 16 bytes (x, y, width, height)
/// - bull_color: 16 bytes (RGBA color for bullish candles)
/// - bear_color: 16 bytes (RGBA color for bearish candles)
/// - wick_color: 16 bytes (RGBA color for candle wicks)
///
/// # GPU Compatibility
/// - Uses #[repr(C)] for predictable memory layout
/// - 16-byte alignment for uniform buffer requirements
/// - All fields are f32 arrays (valid for any bit pattern)
///
/// # Usage
/// This struct is uploaded to the GPU once per frame and shared by all
/// candlestick instances. The shader reads these values to transform vertices
/// and apply colors based on the is_bullish flag.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ViewUniform {
    /// 4x4 orthographic projection matrix for transforming world coordinates to clip space
    /// Layout: column-major, suitable for GPU consumption
    pub view_proj: [[f32; 4]; 4],

    /// Viewport parameters: [x, y, width, height]
    /// Used for coordinate transformations and aspect ratio calculations
    pub viewport: [f32; 4],

    /// Bullish candle color (RGBA), typically green
    /// Applied when is_bullish = 1.0
    pub bull_color: [f32; 4],

    /// Bearish candle color (RGBA), typically red
    /// Applied when is_bullish = 0.0
    pub bear_color: [f32; 4],

    /// Candle wick color (RGBA)
    /// Applied to the high-low line segments
    pub wick_color: [f32; 4],
}

// Compile-time assertions to verify ViewUniform layout requirements
const _: () = {
    // Verify total size is exactly 128 bytes
    assert!(
        std::mem::size_of::<ViewUniform>() == 128,
        "ViewUniform must be exactly 128 bytes"
    );

    // Verify alignment meets GPU uniform buffer requirements (16 bytes)
    assert!(
        std::mem::align_of::<ViewUniform>() >= 4,
        "ViewUniform must have at least 4-byte alignment"
    );
};

/// GPU-compatible shader configuration (16 bytes)
///
/// This struct contains configurable parameters for the candlestick shader,
/// allowing runtime adjustment of rendering properties without shader recompilation.
///
/// # Memory Layout (16 bytes)
/// - wick_width_ratio: Wick width as fraction of candle width (default: 0.15)
/// - min_wick_width: Minimum wick width in pixels (default: 2.0)
/// - min_element_height: Minimum height for body/wick in pixels (default: 1.0)
/// - _padding: Padding for 16-byte alignment
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ChartConfig {
    /// Wick width as a fraction of candle body width (0.0-1.0)
    pub wick_width_ratio: f32,

    /// Minimum wick width in pixels
    pub min_wick_width: f32,

    /// Minimum element height in pixels (for body and wick)
    pub min_element_height: f32,

    /// Padding for 16-byte GPU alignment
    pub _padding: f32,
}

// Compile-time assertion to verify ChartConfig is exactly 16 bytes
const _: () = {
    assert!(
        std::mem::size_of::<ChartConfig>() == 16,
        "ChartConfig must be exactly 16 bytes"
    );
};

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            wick_width_ratio: 0.15,
            min_wick_width: 2.0,
            min_element_height: 1.0,
            _padding: 0.0,
        }
    }
}

impl ChartConfig {
    /// Create a new ChartConfig with custom values
    pub fn new(wick_width_ratio: f32, min_wick_width: f32, min_element_height: f32) -> Self {
        Self {
            wick_width_ratio,
            min_wick_width,
            min_element_height,
            _padding: 0.0,
        }
    }
}

impl ViewUniform {
    /// Create a new ViewUniform with identity matrix and default colors
    ///
    /// # Default Values
    /// - view_proj: Identity matrix (no transformation)
    /// - viewport: [0.0, 0.0, 800.0, 600.0]
    /// - bull_color: [0.0, 1.0, 0.0, 1.0] (green)
    /// - bear_color: [1.0, 0.0, 0.0, 1.0] (red)
    /// - wick_color: [0.5, 0.5, 0.5, 1.0] (gray)
    pub fn new() -> Self {
        Self {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            viewport: [0.0, 0.0, 800.0, 600.0],
            bull_color: [0.0, 1.0, 0.0, 1.0],
            bear_color: [1.0, 0.0, 0.0, 1.0],
            wick_color: [0.5, 0.5, 0.5, 1.0],
        }
    }

    /// Set the orthographic projection matrix
    ///
    /// # Arguments
    /// * `matrix` - 4x4 column-major projection matrix
    pub fn with_projection(mut self, matrix: [[f32; 4]; 4]) -> Self {
        self.view_proj = matrix;
        self
    }

    /// Set the viewport parameters
    ///
    /// # Arguments
    /// * `x` - Viewport X offset
    /// * `y` - Viewport Y offset
    /// * `width` - Viewport width
    /// * `height` - Viewport height
    pub fn with_viewport(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.viewport = [x, y, width, height];
        self
    }

    /// Set custom colors for bullish and bearish candles
    ///
    /// # Arguments
    /// * `bull_color` - RGBA color for bullish candles
    /// * `bear_color` - RGBA color for bearish candles
    /// * `wick_color` - RGBA color for wicks
    pub fn with_colors(
        mut self,
        bull_color: [f32; 4],
        bear_color: [f32; 4],
        wick_color: [f32; 4],
    ) -> Self {
        self.bull_color = bull_color;
        self.bear_color = bear_color;
        self.wick_color = wick_color;
        self
    }
}

/// GPU-compatible instance data for a single volume bar (32 bytes)
///
/// This struct is designed for instanced rendering of volume bars on the GPU.
/// It stores absolute world coordinates for proper positioning.
///
/// # Memory Layout (32 bytes total)
/// - x_position: horizontal position (bar center)
/// - width: bar width in pixels
/// - y_bottom: bottom y position in world space
/// - y_top: top y position in world space
/// - is_bullish: 1.0 for bullish (green), 0.0 for bearish (red)
/// - _padding: 3 floats to align to 32 bytes
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct VolumeInstance {
    /// Horizontal position in world space (bar center)
    pub x_position: f32,

    /// Width of the volume bar in pixels
    pub width: f32,

    /// Bottom Y position in world space
    pub y_bottom: f32,

    /// Top Y position in world space
    pub y_top: f32,

    /// Bullish indicator: 1.0 = bullish (green), 0.0 = bearish (red)
    pub is_bullish: f32,

    /// Padding for 32-byte alignment
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
}

// Compile-time assertion to verify VolumeInstance is exactly 32 bytes
const _: () = {
    assert!(
        std::mem::size_of::<VolumeInstance>() == 32,
        "VolumeInstance must be exactly 32 bytes"
    );
};

impl VolumeInstance {
    /// Create a new VolumeInstance
    ///
    /// # Arguments
    /// * `x_position` - Horizontal position (bar center)
    /// * `width` - Bar width in pixels
    /// * `y_bottom` - Bottom Y position in world space
    /// * `y_top` - Top Y position in world space
    /// * `is_bullish` - True for bullish (green), false for bearish (red)
    pub fn new(x_position: f32, width: f32, y_bottom: f32, y_top: f32, is_bullish: bool) -> Self {
        Self {
            x_position,
            width,
            y_bottom,
            y_top,
            is_bullish: if is_bullish { 1.0 } else { 0.0 },
            _padding0: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
        }
    }

    /// Returns the vertex buffer layout for VolumeInstance
    ///
    /// Uses two vec4 attributes:
    /// - Location 0: (x_position, width, y_bottom, y_top)
    /// - Location 1: (is_bullish, padding, padding, padding)
    pub fn vertex_buffer_layout() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<VolumeInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 1,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_and_alignment() {
        // Verify struct size
        assert_eq!(std::mem::size_of::<CandleInstance>(), 32);

        // Verify natural alignment for f32 fields
        assert_eq!(std::mem::align_of::<CandleInstance>(), 4);
    }

    #[test]
    fn test_new_bullish() {
        let candle = CandleInstance::new(100.0, 8.0, 50.0, 52.0, 49.0, 51.0);

        assert_eq!(candle.x_position, 100.0);
        assert_eq!(candle.width, 8.0);
        assert_eq!(candle.open, 50.0);
        assert_eq!(candle.high, 52.0);
        assert_eq!(candle.low, 49.0);
        assert_eq!(candle.close, 51.0);
        assert_eq!(candle.is_bullish, 1.0); // close > open
        assert_eq!(candle._padding, 0.0);
    }

    #[test]
    fn test_new_bearish() {
        let candle = CandleInstance::new(100.0, 8.0, 50.0, 52.0, 48.0, 49.0);

        assert_eq!(candle.is_bullish, 0.0); // close < open
    }

    #[test]
    fn test_new_doji() {
        let candle = CandleInstance::new(100.0, 8.0, 50.0, 52.0, 49.0, 50.0);

        assert_eq!(candle.is_bullish, 1.0); // close == open, treated as bullish
    }

    #[test]
    fn test_with_bullish_override() {
        // Force bearish even though close > open
        let candle = CandleInstance::with_bullish(100.0, 8.0, 50.0, 52.0, 49.0, 51.0, false);

        assert_eq!(candle.is_bullish, 0.0);

        // Force bullish even though close < open
        let candle = CandleInstance::with_bullish(100.0, 8.0, 50.0, 52.0, 48.0, 49.0, true);

        assert_eq!(candle.is_bullish, 1.0);
    }

    #[test]
    fn test_pod_zeroable() {
        // Test that we can create a zeroed instance
        let zeroed: CandleInstance = Zeroable::zeroed();

        assert_eq!(zeroed.x_position, 0.0);
        assert_eq!(zeroed.width, 0.0);
        assert_eq!(zeroed.open, 0.0);
        assert_eq!(zeroed.high, 0.0);
        assert_eq!(zeroed.low, 0.0);
        assert_eq!(zeroed.close, 0.0);
        assert_eq!(zeroed.is_bullish, 0.0);
        assert_eq!(zeroed._padding, 0.0);
    }

    #[test]
    fn test_byte_conversion() {
        let candle = CandleInstance::new(100.0, 8.0, 50.0, 52.0, 49.0, 51.0);

        // Convert to bytes and back
        let bytes: &[u8] = bytemuck::bytes_of(&candle);
        assert_eq!(bytes.len(), 32);

        let restored: &CandleInstance = bytemuck::from_bytes(bytes);
        assert_eq!(restored.x_position, candle.x_position);
        assert_eq!(restored.close, candle.close);
    }

    #[test]
    fn test_vertex_buffer_layout_packed() {
        let layout = CandleInstance::vertex_buffer_layout_packed();

        // Verify array stride
        assert_eq!(layout.array_stride, 32);

        // Verify step mode
        assert!(matches!(layout.step_mode, VertexStepMode::Instance));

        // Verify we have exactly 2 attributes
        assert_eq!(layout.attributes.len(), 2);

        // Verify first attribute (location 0)
        let attr0 = &layout.attributes[0];
        assert_eq!(attr0.shader_location, 0);
        assert_eq!(attr0.offset, 0);
        assert!(matches!(attr0.format, VertexFormat::Float32x4));

        // Verify second attribute (location 1)
        let attr1 = &layout.attributes[1];
        assert_eq!(attr1.shader_location, 1);
        assert_eq!(attr1.offset, 16);
        assert!(matches!(attr1.format, VertexFormat::Float32x4));
    }

    #[test]
    fn test_vertex_buffer_layout_unpacked() {
        let layout = CandleInstance::vertex_buffer_layout_unpacked();

        // Verify array stride
        assert_eq!(layout.array_stride, 32);

        // Verify step mode
        assert!(matches!(layout.step_mode, VertexStepMode::Instance));

        // Verify we have exactly 7 attributes (excluding padding)
        assert_eq!(layout.attributes.len(), 7);

        // Verify shader locations and offsets
        for (i, attr) in layout.attributes.iter().enumerate() {
            assert_eq!(attr.shader_location, i as u32);
            assert_eq!(attr.offset, (i * 4) as u64);
            assert!(matches!(attr.format, VertexFormat::Float32));
        }
    }

    #[test]
    fn test_view_uniform_size_and_alignment() {
        // Verify struct size
        assert_eq!(std::mem::size_of::<ViewUniform>(), 128);

        // Verify alignment (at least 4 bytes for f32 arrays)
        assert!(std::mem::align_of::<ViewUniform>() >= 4);
    }

    #[test]
    fn test_view_uniform_new() {
        let view_uniform = ViewUniform::new();

        // Verify identity matrix
        assert_eq!(view_uniform.view_proj[0], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(view_uniform.view_proj[1], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(view_uniform.view_proj[2], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(view_uniform.view_proj[3], [0.0, 0.0, 0.0, 1.0]);

        // Verify default viewport
        assert_eq!(view_uniform.viewport, [0.0, 0.0, 800.0, 600.0]);

        // Verify default colors
        assert_eq!(view_uniform.bull_color, [0.0, 1.0, 0.0, 1.0]); // green
        assert_eq!(view_uniform.bear_color, [1.0, 0.0, 0.0, 1.0]); // red
        assert_eq!(view_uniform.wick_color, [0.5, 0.5, 0.5, 1.0]); // gray
    }

    #[test]
    fn test_view_uniform_with_projection() {
        let custom_matrix = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        let view_uniform = ViewUniform::new().with_projection(custom_matrix);

        assert_eq!(view_uniform.view_proj, custom_matrix);
    }

    #[test]
    fn test_view_uniform_with_viewport() {
        let view_uniform = ViewUniform::new().with_viewport(10.0, 20.0, 1920.0, 1080.0);

        assert_eq!(view_uniform.viewport, [10.0, 20.0, 1920.0, 1080.0]);
    }

    #[test]
    fn test_view_uniform_with_colors() {
        let custom_bull = [0.2, 0.8, 0.2, 1.0];
        let custom_bear = [0.9, 0.1, 0.1, 1.0];
        let custom_wick = [0.3, 0.3, 0.3, 1.0];

        let view_uniform =
            ViewUniform::new().with_colors(custom_bull, custom_bear, custom_wick);

        assert_eq!(view_uniform.bull_color, custom_bull);
        assert_eq!(view_uniform.bear_color, custom_bear);
        assert_eq!(view_uniform.wick_color, custom_wick);
    }

    #[test]
    fn test_view_uniform_pod_zeroable() {
        // Test that we can create a zeroed instance
        let zeroed: ViewUniform = Zeroable::zeroed();

        // Verify all fields are zero
        for row in &zeroed.view_proj {
            for &val in row {
                assert_eq!(val, 0.0);
            }
        }
        assert_eq!(zeroed.viewport, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(zeroed.bull_color, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(zeroed.bear_color, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(zeroed.wick_color, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_view_uniform_byte_conversion() {
        let view_uniform = ViewUniform::new();

        // Convert to bytes and back
        let bytes: &[u8] = bytemuck::bytes_of(&view_uniform);
        assert_eq!(bytes.len(), 128);

        let restored: &ViewUniform = bytemuck::from_bytes(bytes);
        assert_eq!(restored.view_proj, view_uniform.view_proj);
        assert_eq!(restored.viewport, view_uniform.viewport);
        assert_eq!(restored.bull_color, view_uniform.bull_color);
        assert_eq!(restored.bear_color, view_uniform.bear_color);
        assert_eq!(restored.wick_color, view_uniform.wick_color);
    }

    #[test]
    fn test_view_uniform_builder_chain() {
        let custom_matrix = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let custom_bull = [0.2, 0.8, 0.2, 1.0];
        let custom_bear = [0.9, 0.1, 0.1, 1.0];
        let custom_wick = [0.3, 0.3, 0.3, 1.0];

        // Test chaining multiple builder methods
        let view_uniform = ViewUniform::new()
            .with_projection(custom_matrix)
            .with_viewport(100.0, 200.0, 1920.0, 1080.0)
            .with_colors(custom_bull, custom_bear, custom_wick);

        assert_eq!(view_uniform.view_proj, custom_matrix);
        assert_eq!(view_uniform.viewport, [100.0, 200.0, 1920.0, 1080.0]);
        assert_eq!(view_uniform.bull_color, custom_bull);
        assert_eq!(view_uniform.bear_color, custom_bear);
        assert_eq!(view_uniform.wick_color, custom_wick);
    }
}
