//! Atomic performance statistics for cache tiers
//!
//! This module provides lock-free performance tracking and statistics collection
//! for cache operations with atomic counters and performance scoring.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use crossbeam_utils::CachePadded;

use super::types::TierStatsSnapshot;
use crate::cache::tier::warm::atomic_float::AtomicF64;
use crate::cache::types::timestamp_nanos;

/// Atomic tier statistics for performance monitoring
#[derive(Debug)]
pub struct AtomicTierStats {
    /// Current number of entries
    entry_count: CachePadded<AtomicUsize>,
    /// Current memory usage in bytes
    memory_usage: CachePadded<AtomicU64>,
    /// Total cache hits
    total_hits: CachePadded<AtomicU64>,
    /// Total cache misses
    total_misses: CachePadded<AtomicU64>,
    /// Cache hit rate (moving average)
    hit_rate: AtomicF64,
    /// Average access latency in nanoseconds
    avg_access_latency_ns: AtomicF64,
    /// Peak access latency observed
    peak_access_latency_ns: CachePadded<AtomicU64>,
    /// Operations per second
    ops_per_second: AtomicF64,
    /// Last statistics update timestamp
    last_update_ns: CachePadded<AtomicU64>,
    /// Performance score (0.0-1.0)
    performance_score: AtomicF64,
}

impl AtomicTierStats {
    /// Create new atomic tier statistics
    pub fn new() -> Self {
        Self {
            entry_count: CachePadded::new(AtomicUsize::new(0)),
            memory_usage: CachePadded::new(AtomicU64::new(0)),
            total_hits: CachePadded::new(AtomicU64::new(0)),
            total_misses: CachePadded::new(AtomicU64::new(0)),
            hit_rate: AtomicF64::new(0.0),
            avg_access_latency_ns: AtomicF64::new(0.0),
            peak_access_latency_ns: CachePadded::new(AtomicU64::new(0)),
            ops_per_second: AtomicF64::new(0.0),
            last_update_ns: CachePadded::new(AtomicU64::new(timestamp_nanos(Instant::now()))),
            performance_score: AtomicF64::new(1.0),
        }
    }

    /// Update entry count
    pub fn update_entry_count(&self, delta: i32) {
        if delta > 0 {
            self.entry_count
                .fetch_add(delta as usize, Ordering::Relaxed);
        } else {
            self.entry_count
                .fetch_sub((-delta) as usize, Ordering::Relaxed);
        }
    }

    /// Update memory usage
    pub fn update_memory_usage(&self, delta: i64) {
        if delta > 0 {
            self.memory_usage.fetch_add(delta as u64, Ordering::Relaxed);
        } else {
            self.memory_usage
                .fetch_sub((-delta) as u64, Ordering::Relaxed);
        }
    }

    /// Record cache hit
    pub fn record_hit(&self, access_latency_ns: u64) {
        self.total_hits.fetch_add(1, Ordering::Relaxed);
        self.peak_access_latency_ns
            .fetch_max(access_latency_ns, Ordering::Relaxed);

        // Update average latency using exponential moving average
        let old_avg = self.avg_access_latency_ns.load(Ordering::Relaxed);
        let new_avg = if old_avg == 0.0 {
            access_latency_ns as f64
        } else {
            old_avg * 0.95 + (access_latency_ns as f64) * 0.05
        };
        self.avg_access_latency_ns.store(new_avg, Ordering::Relaxed);

        self.update_hit_rate();
        self.update_performance_metrics();
    }

    /// Record cache miss
    pub fn record_miss(&self, access_latency_ns: u64) {
        self.total_misses.fetch_add(1, Ordering::Relaxed);
        self.peak_access_latency_ns
            .fetch_max(access_latency_ns, Ordering::Relaxed);

        self.update_hit_rate();
        self.update_performance_metrics();
    }

    /// Update hit rate calculation
    fn update_hit_rate(&self) {
        let hits = self.total_hits.load(Ordering::Relaxed);
        let misses = self.total_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total > 0 {
            let hit_rate = hits as f64 / total as f64;
            self.hit_rate.store(hit_rate, Ordering::Relaxed);
        }
    }

    /// Update performance metrics
    fn update_performance_metrics(&self) {
        let current_time = timestamp_nanos(Instant::now());
        let last_update = self.last_update_ns.swap(current_time, Ordering::Relaxed);
        let time_delta_sec = (current_time - last_update) as f64 / 1e9;

        if time_delta_sec > 0.0 {
            // Calculate operations per second
            let total_ops =
                self.total_hits.load(Ordering::Relaxed) + self.total_misses.load(Ordering::Relaxed);
            let ops_rate = total_ops as f64 / time_delta_sec;
            self.ops_per_second.store(ops_rate, Ordering::Relaxed);

            // Calculate performance score (combination of hit rate and latency)
            let hit_rate = self.hit_rate.load(Ordering::Relaxed);
            let avg_latency = self.avg_access_latency_ns.load(Ordering::Relaxed);
            let latency_score = if avg_latency > 0.0 {
                1.0 / (1.0 + avg_latency / 1e6) // Normalize latency (lower is better)
            } else {
                1.0
            };

            let performance = (hit_rate * 0.7) + (latency_score * 0.3);
            self.performance_score.store(performance, Ordering::Relaxed);
        }
    }

    /// Get current statistics snapshot
    pub fn snapshot(&self) -> TierStatsSnapshot {
        TierStatsSnapshot {
            entry_count: self.entry_count.load(Ordering::Relaxed),
            memory_usage: self.memory_usage.load(Ordering::Relaxed),
            total_hits: self.total_hits.load(Ordering::Relaxed),
            total_misses: self.total_misses.load(Ordering::Relaxed),
            hit_rate: self.hit_rate.load(Ordering::Relaxed),
            avg_access_latency_ns: self.avg_access_latency_ns.load(Ordering::Relaxed),
            peak_access_latency_ns: self.peak_access_latency_ns.load(Ordering::Relaxed),
            ops_per_second: self.ops_per_second.load(Ordering::Relaxed),
            performance_score: self.performance_score.load(Ordering::Relaxed),
        }
    }

    /// Get hit rate
    pub fn get_hit_rate(&self) -> f64 {
        self.hit_rate.load(Ordering::Relaxed)
    }

    /// Get performance score
    pub fn get_performance_score(&self) -> f64 {
        self.performance_score.load(Ordering::Relaxed)
    }

    /// Get total operations count
    pub fn get_total_operations(&self) -> u64 {
        self.total_hits.load(Ordering::Relaxed) + self.total_misses.load(Ordering::Relaxed)
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.entry_count.store(0, Ordering::Relaxed);
        self.memory_usage.store(0, Ordering::Relaxed);
        self.total_hits.store(0, Ordering::Relaxed);
        self.total_misses.store(0, Ordering::Relaxed);
        self.hit_rate.store(0.0, Ordering::Relaxed);
        self.avg_access_latency_ns.store(0.0, Ordering::Relaxed);
        self.peak_access_latency_ns.store(0, Ordering::Relaxed);
        self.ops_per_second.store(0.0, Ordering::Relaxed);
        self.last_update_ns
            .store(timestamp_nanos(Instant::now()), Ordering::Relaxed);
        self.performance_score.store(1.0, Ordering::Relaxed);
    }
}
