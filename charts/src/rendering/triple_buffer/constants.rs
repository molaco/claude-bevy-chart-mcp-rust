//! Constants for triple buffering configuration.
//!
//! These constants define the fundamental parameters of the triple buffering system.

/// Number of buffer slots for triple buffering
pub const BUFFER_COUNT: usize = 3;

/// Number of frames to wait before considering a slot "safe" for reuse
/// With triple buffering, after 2 frames the GPU should be done with a buffer
pub const FRAMES_UNTIL_SAFE: u64 = 2;

/// Growth factor for buffer capacity when reallocation is needed.
/// Using 1.5x provides a good balance between memory usage and reallocation frequency.
/// With 1.5x, after n reallocations the buffer size is 1.5^n times the original.
pub const BUFFER_GROWTH_FACTOR: f32 = 1.5;

/// Minimum buffer capacity in bytes to avoid frequent small reallocations.
/// Set to 4KB (a typical page size) as a reasonable minimum.
pub const MIN_BUFFER_CAPACITY: usize = 4096;

/// Number of frames to keep in timing history for debugging
pub const DEBUG_HISTORY_SIZE: usize = 60;
