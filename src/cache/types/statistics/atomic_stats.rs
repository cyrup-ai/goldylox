//! Atomic tier statistics with cache-line alignment
//!
//! This module provides lock-free atomic statistics tracking
//! for high-performance cache tier monitoring.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use crossbeam_utils::CachePadded;

use super::tier_stats::TierStatistics;
use crate::cache::tier::warm::atomic_float::AtomicF64;
use crate::cache::types::timestamp_nanos;

/// Atomic tier statistics with cache-line alignment
#[derive(Debug)]
#[repr(align(64))]
pub struct AtomicTierStats {
    /// Total number of entries
    entry_count: CachePadded<AtomicU32>,
    /// Memory usage in bytes
    memory_bytes: CachePadded<AtomicU64>,
    /// Total access count
    total_accesses: CachePadded<AtomicU64>,
    /// Cache hits
    hits: CachePadded<AtomicU64>,
    /// Cache misses
    misses: CachePadded<AtomicU64>,
    /// Total operation time in nanoseconds
    total_operation_time_ns: CachePadded<AtomicU64>,
    /// Error count
    errors: CachePadded<AtomicU32>,
    /// Last reset timestamp
    last_reset_ns: CachePadded<AtomicU64>,
    /// Peak memory usage (from hot tier version)
    peak_memory: CachePadded<AtomicU64>,
    /// Peak access latency observed (from warm tier version)
    peak_access_latency_ns: CachePadded<AtomicU64>,
    /// Hit rate (moving average) - from warm tier version
    hit_rate: AtomicF64,
    /// Performance score (0.0-1.0) - from warm tier version
    performance_score: AtomicF64,
    /// Total uncompressed data size (for compression ratio calculation)
    total_uncompressed_bytes: CachePadded<AtomicU64>,
}

#[allow(dead_code)] // Complete atomic statistics implementation
impl AtomicTierStats {
    /// Create new atomic statistics
    #[inline(always)]
    pub fn new() -> Self {
        let now_ns = timestamp_nanos(Instant::now());

        Self {
            entry_count: CachePadded::new(AtomicU32::new(0)),
            memory_bytes: CachePadded::new(AtomicU64::new(0)),
            total_accesses: CachePadded::new(AtomicU64::new(0)),
            hits: CachePadded::new(AtomicU64::new(0)),
            misses: CachePadded::new(AtomicU64::new(0)),
            total_operation_time_ns: CachePadded::new(AtomicU64::new(0)),
            errors: CachePadded::new(AtomicU32::new(0)),
            last_reset_ns: CachePadded::new(AtomicU64::new(now_ns)),
            peak_memory: CachePadded::new(AtomicU64::new(0)),
            peak_access_latency_ns: CachePadded::new(AtomicU64::new(0)),
            hit_rate: AtomicF64::new(0.0),
            performance_score: AtomicF64::new(1.0),
            total_uncompressed_bytes: CachePadded::new(AtomicU64::new(0)),
        }
    }

    /// Record cache hit atomically
    #[inline(always)]
    pub fn record_hit(&self, operation_time_ns: u64) {
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.total_operation_time_ns
            .fetch_add(operation_time_ns, Ordering::Relaxed);

        // Update peak access latency
        self.peak_access_latency_ns
            .fetch_max(operation_time_ns, Ordering::Relaxed);

        // Update performance metrics
        self.update_performance_metrics();
    }

    /// Record cache miss atomically
    #[inline(always)]
    pub fn record_miss(&self, operation_time_ns: u64) {
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.total_operation_time_ns
            .fetch_add(operation_time_ns, Ordering::Relaxed);

        // Update peak access latency
        self.peak_access_latency_ns
            .fetch_max(operation_time_ns, Ordering::Relaxed);

        // Update performance metrics
        self.update_performance_metrics();
    }

    /// Record error atomically
    #[inline(always)]
    pub fn record_error(&self, operation_time_ns: u64) {
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
        self.errors.fetch_add(1, Ordering::Relaxed);
        self.total_operation_time_ns
            .fetch_add(operation_time_ns, Ordering::Relaxed);

        // Update peak access latency
        self.peak_access_latency_ns
            .fetch_max(operation_time_ns, Ordering::Relaxed);

        // Update performance metrics for consistency
        self.update_performance_metrics();
    }

    /// Update entry count atomically
    #[inline(always)]
    pub fn update_entry_count(&self, delta: i32) {
        if delta >= 0 {
            self.entry_count.fetch_add(delta as u32, Ordering::Relaxed);
        } else {
            self.entry_count
                .fetch_sub((-delta) as u32, Ordering::Relaxed);
        }
    }

    /// Update memory usage atomically
    #[inline(always)]
    pub fn update_memory_usage(&self, delta: i64) {
        let current = if delta >= 0 {
            self.memory_bytes.fetch_add(delta as u64, Ordering::Relaxed) + delta as u64
        } else {
            self.memory_bytes
                .fetch_sub((-delta) as u64, Ordering::Relaxed)
                - (-delta) as u64
        };

        // Update peak memory if necessary (from hot tier version)
        let mut peak = self.peak_memory.load(Ordering::Relaxed);
        while current > peak {
            match self.peak_memory.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    /// Get current statistics snapshot
    #[inline(always)]
    pub fn snapshot(&self) -> TierStatistics {
        let entries = self.entry_count.load(Ordering::Relaxed);
        let memory = self.memory_bytes.load(Ordering::Relaxed);
        let total_accesses = self.total_accesses.load(Ordering::Relaxed);
        let hits = self.hits.load(Ordering::Relaxed);
        let _misses = self.misses.load(Ordering::Relaxed);
        let total_time_ns = self.total_operation_time_ns.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);

        // Calculate derived statistics
        let hit_rate = if total_accesses > 0 {
            hits as f64 / total_accesses as f64
        } else {
            0.0
        };

        let avg_access_time_ns = if total_accesses > 0 {
            total_time_ns / total_accesses
        } else {
            0
        };

        let error_rate = if total_accesses > 0 {
            errors as f64 / total_accesses as f64
        } else {
            0.0
        };

        TierStatistics {
            hits,
            misses: _misses,
            entry_count: entries as usize,
            memory_usage: memory as usize,
            peak_memory: self.peak_memory.load(Ordering::Relaxed),
            total_size_bytes: self.total_uncompressed_bytes.load(Ordering::Relaxed),
            hit_rate,
            avg_access_time_ns,
            ops_per_second: Self::calculate_ops_per_second(total_accesses, self.uptime_ns()),
            error_count: errors as u64,
            error_rate,
        }
    }

    /// Calculate operations per second
    #[inline(always)]
    fn calculate_ops_per_second(total_ops: u64, uptime_ns: u64) -> f64 {
        if uptime_ns > 0 {
            (total_ops as f64) / (uptime_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        }
    }

    /// Get uptime in nanoseconds
    #[inline(always)]
    fn uptime_ns(&self) -> u64 {
        let last_reset = self.last_reset_ns.load(Ordering::Relaxed);
        timestamp_nanos(Instant::now()).saturating_sub(last_reset)
    }

    /// Get current memory usage in bytes
    #[inline(always)]
    pub fn memory_usage_bytes(&self) -> u64 {
        self.memory_bytes.load(Ordering::Relaxed)
    }

    /// Reset all statistics
    #[inline(always)]
    pub fn reset(&self) {
        let now_ns = timestamp_nanos(Instant::now());

        self.entry_count.store(0, Ordering::Relaxed);
        self.memory_bytes.store(0, Ordering::Relaxed);
        self.total_accesses.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.total_operation_time_ns.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.last_reset_ns.store(now_ns, Ordering::Relaxed);
        self.peak_memory.store(0, Ordering::Relaxed);
        self.peak_access_latency_ns.store(0, Ordering::Relaxed);
        self.hit_rate.store(0.0, Ordering::Relaxed);
        self.performance_score.store(1.0, Ordering::Relaxed);
        self.total_uncompressed_bytes.store(0, Ordering::Relaxed);
    }

    /// Update performance metrics (from warm tier version)
    #[inline(always)]
    fn update_performance_metrics(&self) {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_accesses = hits + misses;

        if total_accesses > 0 {
            // Update hit rate
            let hit_rate = hits as f64 / total_accesses as f64;
            self.hit_rate.store(hit_rate, Ordering::Relaxed);

            // Calculate performance score (combination of hit rate and latency)
            let total_time = self.total_operation_time_ns.load(Ordering::Relaxed);
            let avg_latency = if total_accesses > 0 {
                total_time as f64 / total_accesses as f64
            } else {
                0.0
            };

            let latency_score = if avg_latency > 0.0 {
                1.0 / (1.0 + avg_latency / 1e6) // Normalize latency (lower is better)
            } else {
                1.0
            };

            let performance = (hit_rate * 0.7) + (latency_score * 0.3);
            self.performance_score.store(performance, Ordering::Relaxed);
        }
    }

    /// Get memory efficiency (current/peak ratio) - from hot tier version
    #[inline(always)]
    pub fn memory_efficiency(&self) -> f64 {
        let current = self.memory_bytes.load(Ordering::Relaxed);
        let peak = self.peak_memory.load(Ordering::Relaxed);
        if peak > 0 {
            current as f64 / peak as f64
        } else {
            1.0
        }
    }

    /// Get performance score - from warm tier version
    #[inline(always)]
    pub fn get_performance_score(&self) -> f64 {
        self.performance_score.load(Ordering::Relaxed)
    }

    /// Get current hit rate - from warm tier version
    #[inline(always)]
    pub fn get_hit_rate(&self) -> f64 {
        self.hit_rate.load(Ordering::Relaxed)
    }

    /// Get peak access latency - from warm tier version
    #[inline(always)]
    pub fn get_peak_access_latency_ns(&self) -> u64 {
        self.peak_access_latency_ns.load(Ordering::Relaxed)
    }

    /// Get peak memory usage - from hot tier version
    #[inline(always)]
    pub fn get_peak_memory(&self) -> u64 {
        self.peak_memory.load(Ordering::Relaxed)
    }

    /// Update total uncompressed size atomically
    #[inline(always)]
    pub fn update_uncompressed_size(&self, delta: i64) {
        if delta >= 0 {
            self.total_uncompressed_bytes
                .fetch_add(delta as u64, Ordering::Relaxed);
        } else {
            self.total_uncompressed_bytes
                .fetch_sub((-delta) as u64, Ordering::Relaxed);
        }
    }

    /// Get total uncompressed size
    #[inline(always)]
    pub fn total_uncompressed_bytes(&self) -> u64 {
        self.total_uncompressed_bytes.load(Ordering::Relaxed)
    }
}

impl Default for AtomicTierStats {
    fn default() -> Self {
        Self::new()
    }
}
