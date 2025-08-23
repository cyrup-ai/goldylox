//! Core types and utilities for cold tier cache
//!
//! This module provides essential types and utility functions used throughout
//! the persistent cold tier cache implementation.

use std::time::Instant;

/// Precision timer for performance measurement
#[derive(Debug)]
pub struct PrecisionTimer {
    start: Instant,
}

impl PrecisionTimer {
    /// Start a new precision timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time in nanoseconds
    pub fn elapsed_ns(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }
}

/// Convert Instant to nanoseconds timestamp
pub fn timestamp_nanos(instant: Instant) -> u64 {
    // This is a simplified implementation
    // In practice, you'd want a more robust timestamp system
    instant.elapsed().as_nanos() as u64
}

/// Statistics for cold tier cache performance
#[derive(Debug, Clone)]
pub struct ColdTierStats {
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub compression_ratio: f32,
    pub hit_rate: f32,
    pub avg_access_time_ns: u64,
}
