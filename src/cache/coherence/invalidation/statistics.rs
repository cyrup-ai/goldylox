//! Statistics for invalidation operations
//!
//! This module implements statistics tracking and metrics calculation for
//! invalidation operations including success rates and performance metrics.

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

/// Snapshot of invalidation statistics for monitoring
#[derive(Debug, Clone, Copy)]
pub struct InvalidationStatisticsSnapshot {
    pub total_requested: u64,
    pub successful: u64,
    pub failed: u64,
    pub retried: u64,
    pub timed_out: u64,
    pub avg_processing_time_ns: u64,
    pub peak_pending_count: u32,
    pub current_pending_count: u32,
}

impl InvalidationStatisticsSnapshot {
    /// Calculate success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_requested == 0 {
            0.0
        } else {
            (self.successful as f64 / self.total_requested as f64) * 100.0
        }
    }

    /// Calculate failure rate as percentage
    pub fn failure_rate(&self) -> f64 {
        if self.total_requested == 0 {
            0.0
        } else {
            (self.failed as f64 / self.total_requested as f64) * 100.0
        }
    }

    /// Calculate retry rate as percentage
    pub fn retry_rate(&self) -> f64 {
        if self.total_requested == 0 {
            0.0
        } else {
            (self.retried as f64 / self.total_requested as f64) * 100.0
        }
    }
}
