//! Statistics and monitoring for write propagation system
//!
//! This module implements performance tracking, metrics collection,
//! and monitoring functionality for write propagation operations.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::types::PropagationStatistics;

impl Default for PropagationStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl PropagationStatistics {
    /// Create new statistics instance with zero values
    pub fn new() -> Self {
        Self {
            writebacks: AtomicU64::new(0),
            propagations: AtomicU64::new(0),
            failed_writes: AtomicU64::new(0),
            avg_write_latency_ns: AtomicU64::new(0),
            peak_queue_depth: AtomicU32::new(0),
            total_bytes_written: AtomicU64::new(0),
            low_priority_writes: AtomicU64::new(0),
            normal_priority_writes: AtomicU64::new(0),
            high_priority_writes: AtomicU64::new(0),
            critical_priority_writes: AtomicU64::new(0),
            worker_tasks_processed: AtomicU64::new(0),
            worker_errors: AtomicU64::new(0),
        }
    }

    /// Record a failed write operation
    pub fn record_failed_write(&self) {
        self.failed_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record worker task completion
    pub fn record_worker_task(&self) {
        self.worker_tasks_processed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record worker error
    pub fn record_worker_error(&self) {
        self.worker_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Update write latency with exponential moving average
    pub fn update_write_latency(&self, latency_ns: u64) {
        let current_avg = self.avg_write_latency_ns.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            latency_ns
        } else {
            (current_avg * 7 + latency_ns) / 8 // Exponential moving average
        };
        self.avg_write_latency_ns.store(new_avg, Ordering::Relaxed);
    }

    /// Update peak queue depth if current depth is higher
    pub fn update_peak_queue_depth(&self, current_depth: u32) {
        let current_peak = self.peak_queue_depth.load(Ordering::Relaxed);
        if current_depth > current_peak {
            self.peak_queue_depth
                .store(current_depth, Ordering::Relaxed);
        }
    }

    /// Add bytes to total written counter
    pub fn add_bytes_written(&self, bytes: u64) {
        self.total_bytes_written.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Reset all statistics to zero
    #[allow(dead_code)] // MESI coherence - statistics reset used in system maintenance
    pub fn reset(&self) {
        self.writebacks.store(0, Ordering::Relaxed);
        self.propagations.store(0, Ordering::Relaxed);
        self.failed_writes.store(0, Ordering::Relaxed);
        self.avg_write_latency_ns.store(0, Ordering::Relaxed);
        self.peak_queue_depth.store(0, Ordering::Relaxed);
        self.total_bytes_written.store(0, Ordering::Relaxed);
        self.low_priority_writes.store(0, Ordering::Relaxed);
        self.normal_priority_writes.store(0, Ordering::Relaxed);
        self.high_priority_writes.store(0, Ordering::Relaxed);
        self.critical_priority_writes.store(0, Ordering::Relaxed);
        self.worker_tasks_processed.store(0, Ordering::Relaxed);
        self.worker_errors.store(0, Ordering::Relaxed);
    }
}
