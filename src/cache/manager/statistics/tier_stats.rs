//! TierStatistics implementation
//!
//! This module provides implementation for individual cache tier statistics
//! including hit rate calculation and access time averaging.

use super::types::TierStatistics;

impl TierStatistics {
    /// Create new tier statistics
    pub fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            avg_access_time_ns: 0,
            memory_usage_bytes: 0,
            entry_count: 0,
            hit_rate: 0.0,
        }
    }

    /// Calculate hit rate
    pub fn calculate_hit_rate(&mut self) {
        let total_requests = self.hits + self.misses;
        if total_requests > 0 {
            self.hit_rate = self.hits as f64 / total_requests as f64;
        } else {
            self.hit_rate = 0.0;
        }
    }

    /// Update access time average
    pub fn update_avg_access_time(&mut self, new_time_ns: u64) {
        let total_requests = self.hits + self.misses;
        if total_requests > 0 {
            self.avg_access_time_ns =
                (self.avg_access_time_ns * (total_requests - 1) + new_time_ns) / total_requests;
        } else {
            self.avg_access_time_ns = new_time_ns;
        }
    }
}
