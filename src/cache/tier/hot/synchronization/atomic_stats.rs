//! Atomic tier statistics with memory-ordered operations
//!
//! This module provides thread-safe statistics collection and reporting
//! for cache tier performance monitoring.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Atomic tier statistics with memory-ordered operations
#[derive(Debug)]
pub struct AtomicTierStats {
    /// Cache hit count
    hits: AtomicU64,
    /// Cache miss count
    misses: AtomicU64,
    /// Current entry count
    entry_count: AtomicUsize,
    /// Memory usage in bytes
    memory_usage: AtomicU64,
    /// Peak memory usage
    peak_memory: AtomicU64,
    /// Access latency accumulator (nanoseconds)
    total_access_latency: AtomicU64,
    /// Access count for average calculation
    access_count: AtomicU64,
    /// Error count
    error_count: AtomicU64,
}

impl AtomicTierStats {
    /// Create new atomic statistics
    pub fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            entry_count: AtomicUsize::new(0),
            memory_usage: AtomicU64::new(0),
            peak_memory: AtomicU64::new(0),
            total_access_latency: AtomicU64::new(0),
            access_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }

    /// Record cache hit
    #[inline(always)]
    pub fn record_hit(&self, latency_ns: u64) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.total_access_latency
            .fetch_add(latency_ns, Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache miss
    #[inline(always)]
    pub fn record_miss(&self, latency_ns: u64) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.total_access_latency
            .fetch_add(latency_ns, Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record error
    #[inline(always)]
    pub fn record_error(&self, latency_ns: u64) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.total_access_latency
            .fetch_add(latency_ns, Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Update entry count (positive or negative delta)
    #[inline(always)]
    pub fn update_entry_count(&self, delta: i32) {
        if delta >= 0 {
            self.entry_count
                .fetch_add(delta as usize, Ordering::Relaxed);
        } else {
            self.entry_count
                .fetch_sub((-delta) as usize, Ordering::Relaxed);
        }
    }

    /// Update memory usage (positive or negative delta)
    #[inline(always)]
    pub fn update_memory_usage(&self, delta: i64) {
        let current = if delta >= 0 {
            self.memory_usage.fetch_add(delta as u64, Ordering::Relaxed) + delta as u64
        } else {
            self.memory_usage
                .fetch_sub((-delta) as u64, Ordering::Relaxed)
                - (-delta) as u64
        };

        // Update peak memory if necessary
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
    pub fn snapshot(&self) -> TierStatistics {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let access_count = self.access_count.load(Ordering::Relaxed);
        let total_latency = self.total_access_latency.load(Ordering::Relaxed);

        TierStatistics {
            hits,
            misses,
            hit_rate: if hits + misses > 0 {
                hits as f64 / (hits + misses) as f64
            } else {
                0.0
            },
            entry_count: self.entry_count.load(Ordering::Relaxed),
            memory_usage: self.memory_usage.load(Ordering::Relaxed),
            peak_memory: self.peak_memory.load(Ordering::Relaxed),
            avg_access_latency_ns: if access_count > 0 {
                total_latency / access_count
            } else {
                0
            },
            error_count: self.error_count.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.entry_count.store(0, Ordering::Relaxed);
        let current_memory = self.memory_usage.load(Ordering::Relaxed);
        self.peak_memory.store(current_memory, Ordering::Relaxed);
        self.total_access_latency.store(0, Ordering::Relaxed);
        self.access_count.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
    }

    /// Get memory efficiency (current/peak ratio)
    pub fn memory_efficiency(&self) -> f64 {
        let current = self.memory_usage.load(Ordering::Relaxed);
        let peak = self.peak_memory.load(Ordering::Relaxed);
        if peak > 0 {
            current as f64 / peak as f64
        } else {
            1.0
        }
    }
}

/// Tier statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct TierStatistics {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub entry_count: usize,
    pub memory_usage: u64,
    pub peak_memory: u64,
    pub avg_access_latency_ns: u64,
    pub error_count: u64,
}

impl TierStatistics {
    /// Get cache efficiency score (0.0 to 1.0)
    pub fn efficiency_score(&self) -> f64 {
        let hit_rate_weight = 0.6;
        let latency_weight = 0.3;
        let error_weight = 0.1;

        let hit_score = self.hit_rate;
        let latency_score = if self.avg_access_latency_ns > 0 {
            1.0 / (1.0 + self.avg_access_latency_ns as f64 / 1000.0) // Normalize to microseconds
        } else {
            1.0
        };
        let error_score = if self.hits + self.misses > 0 {
            1.0 - (self.error_count as f64 / (self.hits + self.misses) as f64)
        } else {
            1.0
        };

        hit_rate_weight * hit_score + latency_weight * latency_score + error_weight * error_score
    }

    /// Check if performance is acceptable
    pub fn is_performing_well(&self) -> bool {
        self.hit_rate > 0.8 && self.avg_access_latency_ns < 10_000 && self.error_count == 0
    }

    /// Get memory utilization ratio
    pub fn memory_utilization(&self) -> f64 {
        if self.peak_memory > 0 {
            self.memory_usage as f64 / self.peak_memory as f64
        } else {
            0.0
        }
    }
}
