//! Atomic performance statistics for cache tiers
//!
//! This module provides lock-free performance tracking and statistics collection
//! for cache operations with atomic counters and performance scoring.

#![allow(dead_code)] // Warm tier monitoring - Complete performance statistics library for tier operations

// AtomicTierStats moved to canonical location: crate::cache::types::statistics::atomic_stats::AtomicTierStats
// Use the comprehensive cache-line aligned implementation with enhanced performance metrics

use super::types::TierStatsSnapshot;
use crate::cache::types::statistics::atomic_stats::AtomicTierStats;

impl AtomicTierStats {
    /// Get current statistics snapshot for warm tier
    pub fn warm_snapshot(&self) -> TierStatsSnapshot {
        TierStatsSnapshot {
            entry_count: self.get_entry_count(),
            memory_usage: self.memory_usage_bytes(),
            total_hits: self.get_hit_count(),
            total_misses: self.get_miss_count(),
            hit_rate: self.get_hit_rate(),
            avg_access_latency_ns: self.get_avg_access_latency_ns(),
            peak_access_latency_ns: self.get_peak_access_latency_ns(),
            ops_per_second: self.get_ops_per_second(),
            performance_score: self.get_performance_score(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            memory_pressure: self.memory_efficiency(),
            evictions: self.get_total_operations(),
        }
    }

    /// Get total operations count for warm tier
    pub fn get_total_operations(&self) -> u64 {
        self.get_hit_count() + self.get_miss_count()
    }

    // Helper methods for warm tier snapshot compatibility
    fn get_entry_count(&self) -> usize {
        self.snapshot().entry_count
    }

    fn get_hit_count(&self) -> u64 {
        self.snapshot().hits
    }

    fn get_miss_count(&self) -> u64 {
        self.snapshot().misses
    }

    fn get_avg_access_latency_ns(&self) -> f64 {
        self.snapshot().avg_access_time_ns as f64
    }

    fn get_ops_per_second(&self) -> f64 {
        self.snapshot().ops_per_second
    }
}
