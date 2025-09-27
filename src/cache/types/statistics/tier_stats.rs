//! Tier statistics snapshot and trait implementations
//!
//! This module provides zero-allocation statistics snapshots
//! and trait implementations for cache tier monitoring.

use crate::cache::traits::TierStats;

/// Comprehensive tier statistics snapshot (zero allocation)
/// Combines best features from all tier statistics implementations
#[derive(Debug, Clone, Copy, bincode::Encode, bincode::Decode)]
pub struct TierStatistics {
    /// Explicit hit count
    pub hits: u64,
    /// Explicit miss count  
    pub misses: u64,
    /// Number of entries
    pub entry_count: usize,
    /// Memory usage in bytes
    pub memory_usage: usize,
    /// Peak memory usage in bytes
    pub peak_memory: u64,
    /// Total data size in bytes (for compression ratio calculations)
    pub total_size_bytes: u64,
    /// Hit rate (0.0 to 1.0) - computed from hits/misses
    pub hit_rate: f64,
    /// Average access time in nanoseconds
    pub avg_access_time_ns: u64,
    /// Operations per second
    pub ops_per_second: f64,
    /// Error count
    pub error_count: u64,
    /// Error rate (0.0 to 1.0) - computed from error_count
    pub error_rate: f64,
}

#[allow(dead_code)] // Complete tier statistics implementation
impl TierStatistics {
    /// Merge statistics from another TierStatistics
    pub fn merge(&mut self, other: TierStatistics) {
        // Combine explicit counts
        self.hits += other.hits;
        self.misses += other.misses;
        self.error_count += other.error_count;
        self.entry_count += other.entry_count;
        self.memory_usage += other.memory_usage;
        self.total_size_bytes += other.total_size_bytes;

        // Update peak memory
        self.peak_memory = self.peak_memory.max(other.peak_memory);

        // Recalculate derived metrics
        let total_operations = self.hits + self.misses;
        if total_operations > 0 {
            self.hit_rate = self.hits as f64 / total_operations as f64;
            self.error_rate = self.error_count as f64 / total_operations as f64;
        } else {
            self.hit_rate = 0.0;
            self.error_rate = 0.0;
        }

        // Average access times weighted by operations
        let self_ops = (self.hits + self.misses - other.hits - other.misses) as f64;
        let other_ops = (other.hits + other.misses) as f64;
        let total_ops = self_ops + other_ops;

        if total_ops > 0.0 {
            self.avg_access_time_ns = ((self.avg_access_time_ns as f64 * self_ops
                + other.avg_access_time_ns as f64 * other_ops)
                / total_ops) as u64;
        }

        // Sum operations per second
        self.ops_per_second += other.ops_per_second;
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
        // Use peak memory as capacity if available, otherwise estimate
        if self.peak_memory > 0 {
            self.peak_memory as usize
        } else {
            self.memory_usage * 2
        }
    }

    #[inline(always)]
    fn total_hits(&self) -> u64 {
        // Return explicit hit count instead of calculation
        self.hits
    }

    #[inline(always)]
    fn total_misses(&self) -> u64 {
        // Return explicit miss count instead of calculation
        self.misses
    }

    #[inline(always)]
    fn peak_access_latency_ns(&self) -> u64 {
        // Estimate peak latency as 3x average (reasonable heuristic)
        self.avg_access_time_ns * 3
    }
}

#[allow(dead_code)] // Complete tier statistics implementation
impl TierStatistics {
    /// Create new comprehensive tier statistics
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub fn new(
        hits: u64,
        misses: u64,
        entry_count: usize,
        memory_usage: usize,
        peak_memory: u64,
        total_size_bytes: u64,
        avg_access_time_ns: u64,
        ops_per_second: f64,
        error_count: u64,
    ) -> Self {
        let total_operations = hits + misses;
        let hit_rate = if total_operations > 0 {
            hits as f64 / total_operations as f64
        } else {
            0.0
        };
        let error_rate = if total_operations > 0 {
            error_count as f64 / total_operations as f64
        } else {
            0.0
        };

        Self {
            hits,
            misses,
            entry_count,
            memory_usage,
            peak_memory,
            total_size_bytes,
            hit_rate,
            avg_access_time_ns,
            ops_per_second,
            error_count,
            error_rate,
        }
    }

    /// Create from basic metrics (backward compatibility)
    #[inline(always)]
    pub fn from_basic(
        entry_count: usize,
        memory_usage: usize,
        hit_rate: f64,
        avg_access_time_ns: u64,
        ops_per_second: f64,
        error_rate: f64,
    ) -> Self {
        // Estimate hits/misses from hit_rate and ops_per_second
        let estimated_operations = (ops_per_second * 60.0) as u64; // Assume 1 minute of ops
        let hits = (hit_rate * estimated_operations as f64) as u64;
        let misses = estimated_operations - hits;
        let error_count = (error_rate * estimated_operations as f64) as u64;

        Self {
            hits,
            misses,
            entry_count,
            memory_usage,
            peak_memory: memory_usage as u64,
            total_size_bytes: memory_usage as u64,
            hit_rate,
            avg_access_time_ns,
            ops_per_second,
            error_count,
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

    /// Get compression ratio if total_size_bytes is available
    #[inline(always)]
    pub fn compression_ratio(&self) -> f32 {
        if self.total_size_bytes > 0 && self.memory_usage > 0 {
            self.memory_usage as f32 / self.total_size_bytes as f32
        } else {
            1.0 // No compression
        }
    }

    /// Check if tier is performing well (enhanced version)
    #[inline(always)]
    pub fn is_performing_well(&self) -> bool {
        self.hit_rate > 0.8 && self.avg_access_time_ns < 10_000 && self.error_count == 0
    }

    /// Get memory utilization ratio (current/peak)
    #[inline(always)]
    pub fn memory_utilization(&self) -> f64 {
        if self.peak_memory > 0 {
            self.memory_usage as f64 / self.peak_memory as f64
        } else {
            0.0
        }
    }
}

impl Default for TierStatistics {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            entry_count: 0,
            memory_usage: 0,
            peak_memory: 0,
            total_size_bytes: 0,
            hit_rate: 0.0,
            avg_access_time_ns: 0,
            ops_per_second: 0.0,
            error_count: 0,
            error_rate: 0.0,
        }
    }
}
