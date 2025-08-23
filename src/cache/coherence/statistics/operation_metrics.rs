//! Operation metrics for coherence protocol performance tracking
//!
//! This module provides detailed metrics for different types of coherence
//! operations including reads, writes, invalidations, and state transitions.

use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics for a specific type of operation
#[derive(Debug)]
pub struct OperationMetrics {
    /// Total operations performed
    pub total_count: AtomicU64,
    /// Successful operations
    pub success_count: AtomicU64,
    /// Failed operations
    pub failure_count: AtomicU64,
    /// Total processing time in nanoseconds
    pub total_processing_time_ns: AtomicU64,
    /// Minimum processing time in nanoseconds
    pub min_processing_time_ns: AtomicU64,
    /// Maximum processing time in nanoseconds
    pub max_processing_time_ns: AtomicU64,
}

impl OperationMetrics {
    pub fn new() -> Self {
        Self {
            total_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            total_processing_time_ns: AtomicU64::new(0),
            min_processing_time_ns: AtomicU64::new(u64::MAX),
            max_processing_time_ns: AtomicU64::new(0),
        }
    }

    /// Record a successful operation
    pub fn record_success(&self, processing_time_ns: u64) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_ns
            .fetch_add(processing_time_ns, Ordering::Relaxed);

        // Update min/max processing times
        self.update_min_time(processing_time_ns);
        self.update_max_time(processing_time_ns);
    }

    /// Record a failed operation
    pub fn record_failure(&self, processing_time_ns: u64) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_ns
            .fetch_add(processing_time_ns, Ordering::Relaxed);

        // Update min/max processing times
        self.update_min_time(processing_time_ns);
        self.update_max_time(processing_time_ns);
    }

    fn update_min_time(&self, time_ns: u64) {
        let mut current_min = self.min_processing_time_ns.load(Ordering::Relaxed);
        while time_ns < current_min {
            match self.min_processing_time_ns.compare_exchange_weak(
                current_min,
                time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_min = actual,
            }
        }
    }

    fn update_max_time(&self, time_ns: u64) {
        let mut current_max = self.max_processing_time_ns.load(Ordering::Relaxed);
        while time_ns > current_max {
            match self.max_processing_time_ns.compare_exchange_weak(
                current_max,
                time_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }
    }

    /// Calculate average processing time
    pub fn average_processing_time_ns(&self) -> u64 {
        let total_count = self.total_count.load(Ordering::Relaxed);
        if total_count == 0 {
            0
        } else {
            self.total_processing_time_ns.load(Ordering::Relaxed) / total_count
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.total_count.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            (self.success_count.load(Ordering::Relaxed) as f64 / total as f64) * 100.0
        }
    }
}
