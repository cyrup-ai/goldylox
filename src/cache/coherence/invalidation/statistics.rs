//! Statistics for invalidation operations
//!
//! This module implements statistics tracking and metrics calculation for
//! invalidation operations including success rates and performance metrics.

// Internal invalidation architecture - components may not be used in minimal API

use std::sync::atomic::{AtomicU32, AtomicU64};

/// Statistics for invalidation operations
#[derive(Debug)]
pub struct InvalidationStatistics {
    /// Total invalidations requested
    pub total_requested: AtomicU64,
    /// Successful invalidations
    pub successful: AtomicU64,
    /// Failed invalidations
    pub failed: AtomicU64,
    /// Retried invalidations
    pub retried: AtomicU64,
    /// Timed out invalidations
    pub timed_out: AtomicU64,
    /// Average processing time in nanoseconds
    pub avg_processing_time_ns: AtomicU64,
    /// Peak pending count
    pub peak_pending_count: AtomicU32,
}

impl Default for InvalidationStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl InvalidationStatistics {
    pub fn new() -> Self {
        Self {
            total_requested: AtomicU64::new(0),
            successful: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            retried: AtomicU64::new(0),
            timed_out: AtomicU64::new(0),
            avg_processing_time_ns: AtomicU64::new(0),
            peak_pending_count: AtomicU32::new(0),
        }
    }
}
