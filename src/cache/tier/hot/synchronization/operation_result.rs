//! Operation result types with detailed timing information
//!
//! This module provides result types for cache operations that include
//! performance metrics and timing data.

/// Cache operation result with detailed timing
#[derive(Debug)]
pub struct OperationResult<T> {
    pub result: T,
    pub latency_ns: u64,
    pub generation: u64,
}

impl<T> OperationResult<T> {
    /// Create new operation result
    pub fn new(result: T, latency_ns: u64, generation: u64) -> Self {
        Self {
            result,
            latency_ns,
            generation,
        }
    }

    /// Check if operation was fast (< 1μs)
    pub fn is_fast(&self) -> bool {
        self.latency_ns < 1_000
    }

    /// Check if operation was slow (> 10μs)
    pub fn is_slow(&self) -> bool {
        self.latency_ns > 10_000
    }
}

/// Timestamp conversion utilities  
pub mod timestamp {
    use std::time::Instant;

    /// Convert Instant to nanoseconds timestamp
    #[inline(always)]
    pub fn instant_to_nanos(instant: Instant) -> u64 {
        use std::sync::OnceLock;
        static START_TIME: OnceLock<Instant> = OnceLock::new();

        let start = START_TIME.get_or_init(Instant::now);
        instant.duration_since(*start).as_nanos() as u64
    }

    /// Get current timestamp in nanoseconds
    #[inline(always)]
    pub fn now_nanos() -> u64 {
        instant_to_nanos(Instant::now())
    }
}
