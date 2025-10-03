//! Core coherence statistics tracking
//!
//! This module provides the main CoherenceStatistics struct for comprehensive
//! statistics collection and monitoring of cache coherence protocol performance.

use crossbeam_utils::CachePadded;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Snapshot of coherence statistics for monitoring
#[derive(Debug, Clone, Copy)]
pub struct CoherenceStatisticsSnapshot {
    pub total_transitions: u64,
    pub invalidations_sent: u64,
    pub invalidations_received: u64,
    pub writebacks_performed: u64,
    pub coherence_overhead_ns: u64,
    pub protocol_violations: u32,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub avg_operation_latency_ns: u64,
    pub peak_concurrent_operations: u32,
}

/// Coherence statistics tracking
#[derive(Debug)]
pub struct CoherenceStatistics {
    /// Total state transitions
    pub total_transitions: CachePadded<AtomicU64>,
    /// Invalidations sent
    pub invalidations_sent: CachePadded<AtomicU64>,
    /// Invalidations received
    pub invalidations_received: CachePadded<AtomicU64>,
    /// Write-backs performed
    pub writebacks_performed: CachePadded<AtomicU64>,
    /// Cache coherence overhead in nanoseconds
    pub coherence_overhead_ns: CachePadded<AtomicU64>,
    /// Protocol violations detected
    pub protocol_violations: CachePadded<AtomicU32>,
    /// Successful coherence operations
    pub successful_operations: CachePadded<AtomicU64>,
    /// Failed coherence operations
    pub failed_operations: CachePadded<AtomicU64>,
    /// Average operation latency in nanoseconds
    pub avg_operation_latency_ns: CachePadded<AtomicU64>,
    /// Peak concurrent operations
    pub peak_concurrent_operations: CachePadded<AtomicU32>,
}

impl Default for CoherenceStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl CoherenceStatistics {
    pub fn new() -> Self {
        Self {
            total_transitions: CachePadded::new(AtomicU64::new(0)),
            invalidations_sent: CachePadded::new(AtomicU64::new(0)),
            invalidations_received: CachePadded::new(AtomicU64::new(0)),
            writebacks_performed: CachePadded::new(AtomicU64::new(0)),
            coherence_overhead_ns: CachePadded::new(AtomicU64::new(0)),
            protocol_violations: CachePadded::new(AtomicU32::new(0)),
            successful_operations: CachePadded::new(AtomicU64::new(0)),
            failed_operations: CachePadded::new(AtomicU64::new(0)),
            avg_operation_latency_ns: CachePadded::new(AtomicU64::new(0)),
            peak_concurrent_operations: CachePadded::new(AtomicU32::new(0)),
        }
    }

    /// Record a state transition
    pub fn record_transition(&self) {
        self.total_transitions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an invalidation sent
    pub fn record_invalidation_sent(&self) {
        self.invalidations_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an invalidation received
    pub fn record_invalidation_received(&self) {
        self.invalidations_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a write-back operation
    pub fn record_writeback(&self) {
        self.writebacks_performed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record coherence overhead
    pub fn record_overhead(&self, overhead_ns: u64) {
        self.coherence_overhead_ns
            .fetch_add(overhead_ns, Ordering::Relaxed);
    }

    /// Record a protocol violation
    pub fn record_violation(&self) {
        self.protocol_violations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful operation
    pub fn record_success(&self, latency_ns: u64) {
        self.successful_operations.fetch_add(1, Ordering::Relaxed);
        self.update_average_latency(latency_ns);
    }

    /// Record a failed operation
    pub fn record_failure(&self) {
        self.failed_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Update average operation latency
    fn update_average_latency(&self, latency_ns: u64) {
        let current_avg = self.avg_operation_latency_ns.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            latency_ns
        } else {
            (current_avg * 7 + latency_ns) / 8 // Exponential moving average
        };
        self.avg_operation_latency_ns
            .store(new_avg, Ordering::Relaxed);
    }

    /// Update peak concurrent operations
    pub fn update_concurrent_operations(&self, count: u32) {
        let current_peak = self.peak_concurrent_operations.load(Ordering::Relaxed);
        if count > current_peak {
            self.peak_concurrent_operations
                .store(count, Ordering::Relaxed);
        }
    }

    /// Get statistics snapshot
    pub fn get_snapshot(&self) -> CoherenceStatisticsSnapshot {
        CoherenceStatisticsSnapshot {
            total_transitions: self.total_transitions.load(Ordering::Relaxed),
            invalidations_sent: self.invalidations_sent.load(Ordering::Relaxed),
            invalidations_received: self.invalidations_received.load(Ordering::Relaxed),
            writebacks_performed: self.writebacks_performed.load(Ordering::Relaxed),
            coherence_overhead_ns: self.coherence_overhead_ns.load(Ordering::Relaxed),
            protocol_violations: self.protocol_violations.load(Ordering::Relaxed),
            successful_operations: self.successful_operations.load(Ordering::Relaxed),
            failed_operations: self.failed_operations.load(Ordering::Relaxed),
            avg_operation_latency_ns: self.avg_operation_latency_ns.load(Ordering::Relaxed),
            peak_concurrent_operations: self.peak_concurrent_operations.load(Ordering::Relaxed),
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        let successful = self.successful_operations.load(Ordering::Relaxed);
        let failed = self.failed_operations.load(Ordering::Relaxed);
        let total = successful + failed;

        if total == 0 {
            0.0
        } else {
            (successful as f64 / total as f64) * 100.0
        }
    }

    /// Calculate invalidation efficiency
    pub fn invalidation_efficiency(&self) -> f64 {
        let sent = self.invalidations_sent.load(Ordering::Relaxed);
        let received = self.invalidations_received.load(Ordering::Relaxed);

        if sent == 0 {
            0.0
        } else {
            (received as f64 / sent as f64) * 100.0
        }
    }
}

impl Clone for CoherenceStatistics {
    fn clone(&self) -> Self {
        Self {
            total_transitions: CachePadded::new(AtomicU64::new(
                self.total_transitions.load(Ordering::Relaxed),
            )),
            invalidations_sent: CachePadded::new(AtomicU64::new(
                self.invalidations_sent.load(Ordering::Relaxed),
            )),
            invalidations_received: CachePadded::new(AtomicU64::new(
                self.invalidations_received.load(Ordering::Relaxed),
            )),
            writebacks_performed: CachePadded::new(AtomicU64::new(
                self.writebacks_performed.load(Ordering::Relaxed),
            )),
            coherence_overhead_ns: CachePadded::new(AtomicU64::new(
                self.coherence_overhead_ns.load(Ordering::Relaxed),
            )),
            protocol_violations: CachePadded::new(AtomicU32::new(
                self.protocol_violations.load(Ordering::Relaxed),
            )),
            successful_operations: CachePadded::new(AtomicU64::new(
                self.successful_operations.load(Ordering::Relaxed),
            )),
            failed_operations: CachePadded::new(AtomicU64::new(
                self.failed_operations.load(Ordering::Relaxed),
            )),
            avg_operation_latency_ns: CachePadded::new(AtomicU64::new(
                self.avg_operation_latency_ns.load(Ordering::Relaxed),
            )),
            peak_concurrent_operations: CachePadded::new(AtomicU32::new(
                self.peak_concurrent_operations.load(Ordering::Relaxed),
            )),
        }
    }
}

impl CoherenceStatisticsSnapshot {
    /// Calculate total operations
    pub fn total_operations(&self) -> u64 {
        self.successful_operations + self.failed_operations
    }

    /// Calculate success rate as percentage
    pub fn success_rate(&self) -> f64 {
        let total = self.total_operations();
        if total == 0 {
            0.0
        } else {
            (self.successful_operations as f64 / total as f64) * 100.0
        }
    }

    /// Calculate violation rate as percentage
    pub fn violation_rate(&self) -> f64 {
        let total = self.total_operations();
        if total == 0 {
            0.0
        } else {
            (self.protocol_violations as f64 / total as f64) * 100.0
        }
    }

    /// Calculate average overhead per operation
    pub fn avg_overhead_per_operation_ns(&self) -> u64 {
        let total = self.total_operations();
        if total == 0 {
            0
        } else {
            self.coherence_overhead_ns / total
        }
    }
}
