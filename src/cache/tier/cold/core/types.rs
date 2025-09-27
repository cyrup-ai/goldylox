//! Core types and utilities for cold tier cache
//!
//! This module provides essential types and utility functions used throughout
//! the persistent cold tier cache implementation.

// PrecisionTimer moved to canonical location: crate::cache::types::performance::timer::PrecisionTimer

/// Convert Instant to nanoseconds timestamp (re-export from production timer infrastructure)
pub use crate::cache::types::performance::timer::timestamp_nanos;

/// Statistics for cold tier cache performance
#[allow(dead_code)] // Cold tier - ColdTierStats used in cold tier statistics reporting
#[derive(Debug, Clone)]
pub struct ColdTierStats {
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub compression_ratio: f32,
    pub hit_rate: f32,
    pub avg_access_time_ns: u64,
}
