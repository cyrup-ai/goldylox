//! Precision timing utilities for performance measurement
//!
//! This module provides high-precision timing capabilities for measuring
//! cache operation latencies and performance metrics.

use std::time::{Duration, Instant};

/// Performance timer for sub-nanosecond precision
#[derive(Debug)]
pub struct PrecisionTimer {
    start_time: Instant,
}

impl PrecisionTimer {
    /// Start new precision timer
    #[inline(always)]
    pub fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// Get elapsed time in nanoseconds
    #[inline(always)]
    pub fn elapsed_ns(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }

    /// Get elapsed time as Duration
    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
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

        let start = START_TIME.get_or_init(|| Instant::now());
        instant.duration_since(*start).as_nanos() as u64
    }

    /// Get current timestamp in nanoseconds
    #[inline(always)]
    pub fn now_nanos() -> u64 {
        instant_to_nanos(Instant::now())
    }
}
