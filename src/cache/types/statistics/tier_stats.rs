//! Tier statistics snapshot and trait implementations
//!
//! This module provides zero-allocation statistics snapshots
//! and trait implementations for cache tier monitoring.

use crate::cache::traits::TierStats;

/// Tier statistics snapshot (zero allocation)
#[derive(Debug, Clone, Copy)]
pub struct TierStatistics {
    /// Number of entries
    pub entry_count: usize,
    /// Memory usage in bytes
    pub memory_usage: usize,
    /// Hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Average access time in nanoseconds
    pub avg_access_time_ns: u64,
    /// Operations per second
    pub ops_per_second: f64,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
}

impl TierStatistics {
    /// Merge statistics from another TierStatistics
    pub fn merge(&mut self, other: TierStatistics) {
        self.entry_count += other.entry_count;
        self.memory_usage += other.memory_usage;

        // Average the hit rates weighted by entry count
        let total_entries = self.entry_count + other.entry_count;
        if total_entries > 0 {
            self.hit_rate = (self.hit_rate * self.entry_count as f64
                + other.hit_rate * other.entry_count as f64)
                / total_entries as f64;
        }

        // Average access times
        self.avg_access_time_ns = (self.avg_access_time_ns + other.avg_access_time_ns) / 2;

        // Sum operations per second
        self.ops_per_second += other.ops_per_second;

        // Average error rates
        self.error_rate = (self.error_rate + other.error_rate) / 2.0;
    }
}

impl TierStats for TierStatistics {
    #[inline(always)]
    fn entry_count(&self) -> usize {
        self.entry_count
    }

    #[inline(always)]
    fn memory_usage(&self) -> usize {
        self.memory_usage
    }

    #[inline(always)]
    fn hit_rate(&self) -> f64 {
        self.hit_rate
    }

    #[inline(always)]
    fn avg_access_time_ns(&self) -> u64 {
        self.avg_access_time_ns
    }

    #[inline(always)]
    fn ops_per_second(&self) -> f64 {
        self.ops_per_second
    }

    #[inline(always)]
    fn error_rate(&self) -> f64 {
        self.error_rate
    }

    #[inline(always)]
    fn memory_capacity(&self) -> usize {
        // Return a reasonable default capacity based on current usage
        self.memory_usage * 2
    }

    #[inline(always)]
    fn total_hits(&self) -> u64 {
        // Calculate total hits from hit rate and entry count
        (self.hit_rate * self.entry_count as f64) as u64
    }

    #[inline(always)]
    fn total_misses(&self) -> u64 {
        // Calculate total misses from hit rate and entry count
        ((1.0 - self.hit_rate) * self.entry_count as f64) as u64
    }

    #[inline(always)]
    fn peak_access_latency_ns(&self) -> u64 {
        // Estimate peak latency as 3x average (reasonable heuristic)
        self.avg_access_time_ns * 3
    }
}

impl TierStatistics {
    /// Create new tier statistics
    #[inline(always)]
    pub fn new(
        entry_count: usize,
        memory_usage: usize,
        hit_rate: f64,
        avg_access_time_ns: u64,
        ops_per_second: f64,
        error_rate: f64,
    ) -> Self {
        Self {
            entry_count,
            memory_usage,
            hit_rate,
            avg_access_time_ns,
            ops_per_second,
            error_rate,
        }
    }

    /// Calculate efficiency score (higher is better)
    #[inline(always)]
    pub fn efficiency_score(&self) -> f64 {
        let hit_weight = 0.4;
        let speed_weight = 0.3;
        let reliability_weight = 0.3;

        let speed_score = if self.avg_access_time_ns > 0 {
            1.0 / (self.avg_access_time_ns as f64 / 1_000_000.0) // Inverse of milliseconds
        } else {
            1.0
        };

        let reliability_score = 1.0 - self.error_rate;

        self.hit_rate * hit_weight
            + speed_score.min(1.0) * speed_weight
            + reliability_score * reliability_weight
    }

    /// Check if tier is performing well
    #[inline(always)]
    pub fn is_healthy(&self) -> bool {
        self.hit_rate > 0.8 && self.error_rate < 0.05
    }

    /// Get memory efficiency (entries per MB)
    #[inline(always)]
    pub fn memory_efficiency(&self) -> f64 {
        if self.memory_usage > 0 {
            self.entry_count as f64 / (self.memory_usage as f64 / (1024.0 * 1024.0))
        } else {
            0.0
        }
    }

    /// Get cache utilization percentage
    #[inline(always)]
    pub fn utilization_percentage(&self) -> f64 {
        if self.entry_count > 0 {
            (self.memory_usage as f64 / self.entry_count as f64) * 100.0
        } else {
            0.0
        }
    }
}

impl Default for TierStatistics {
    fn default() -> Self {
        Self {
            entry_count: 0,
            memory_usage: 0,
            hit_rate: 0.0,
            avg_access_time_ns: 0,
            ops_per_second: 0.0,
            error_rate: 0.0,
        }
    }
}
