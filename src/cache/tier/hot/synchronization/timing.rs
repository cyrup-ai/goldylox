//! Precision timing utilities for performance measurement
//!
//! This module provides high-precision timing capabilities for measuring
//! cache operation latencies and performance metrics.

// PrecisionTimer moved to canonical location: crate::cache::types::performance::timer::PrecisionTimer

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
