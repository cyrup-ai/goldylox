//! Pool selection statistics and metrics tracking
//!
//! This module implements the PoolStatistics struct that tracks allocation
//! patterns, hit rates, utilization levels, and efficiency scores across pools.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Pool selection statistics
#[derive(Debug)]
pub struct PoolStatistics {
    /// Allocation counts per pool
    #[allow(dead_code)]
    // Memory management - pool_allocations used in pool allocation statistics
    pub pool_allocations: [AtomicU64; 3], // Small, Medium, Large
    /// Hit rates per pool (successful allocation)
    pub pool_hit_rates: [AtomicU32; 3], // Rate * 1000
    /// Pool utilization levels
    pub pool_utilizations: [AtomicU32; 3], // Percentage * 100
    /// Pool efficiency scores
    pub efficiency_scores: [AtomicU32; 3], // Score * 1000
}

impl Clone for PoolStatistics {
    fn clone(&self) -> Self {
        Self {
            pool_allocations: [
                AtomicU64::new(self.pool_allocations[0].load(Ordering::Relaxed)),
                AtomicU64::new(self.pool_allocations[1].load(Ordering::Relaxed)),
                AtomicU64::new(self.pool_allocations[2].load(Ordering::Relaxed)),
            ],
            pool_hit_rates: [
                AtomicU32::new(self.pool_hit_rates[0].load(Ordering::Relaxed)),
                AtomicU32::new(self.pool_hit_rates[1].load(Ordering::Relaxed)),
                AtomicU32::new(self.pool_hit_rates[2].load(Ordering::Relaxed)),
            ],
            pool_utilizations: [
                AtomicU32::new(self.pool_utilizations[0].load(Ordering::Relaxed)),
                AtomicU32::new(self.pool_utilizations[1].load(Ordering::Relaxed)),
                AtomicU32::new(self.pool_utilizations[2].load(Ordering::Relaxed)),
            ],
            efficiency_scores: [
                AtomicU32::new(self.efficiency_scores[0].load(Ordering::Relaxed)),
                AtomicU32::new(self.efficiency_scores[1].load(Ordering::Relaxed)),
                AtomicU32::new(self.efficiency_scores[2].load(Ordering::Relaxed)),
            ],
        }
    }
}

impl PoolStatistics {
    pub fn new() -> Self {
        Self {
            pool_allocations: [
                AtomicU64::new(0), // Small pool
                AtomicU64::new(0), // Medium pool
                AtomicU64::new(0), // Large pool
            ],
            pool_hit_rates: [
                AtomicU32::new(9500), // 95% hit rate for small pool
                AtomicU32::new(9000), // 90% hit rate for medium pool
                AtomicU32::new(8500), // 85% hit rate for large pool
            ],
            pool_utilizations: [
                AtomicU32::new(7500), // 75% utilization for small pool
                AtomicU32::new(6000), // 60% utilization for medium pool
                AtomicU32::new(4500), // 45% utilization for large pool
            ],
            efficiency_scores: [
                AtomicU32::new(9200), // 92% efficiency for small pool
                AtomicU32::new(8800), // 88% efficiency for medium pool
                AtomicU32::new(8400), // 84% efficiency for large pool
            ],
        }
    }

    #[allow(dead_code)] // Memory management - record_pool_allocation used in pool allocation tracking
    pub fn record_pool_allocation(&self, pool_index: usize) {
        if pool_index < 3 {
            self.pool_allocations[pool_index].fetch_add(1, Ordering::Relaxed);

            // Update utilization based on allocation pattern
            let current_util = self.pool_utilizations[pool_index].load(Ordering::Relaxed);
            let new_util = std::cmp::min(current_util + 10, 10000); // Increase by 0.1%
            self.pool_utilizations[pool_index].store(new_util, Ordering::Relaxed);
        }
    }

    pub fn update_pool_hit_rate(&self, pool_index: usize, was_hit: bool) {
        if pool_index < 3 {
            let current_rate = self.pool_hit_rates[pool_index].load(Ordering::Relaxed);
            let new_rate = if was_hit {
                std::cmp::min(current_rate + 1, 10000)
            } else {
                current_rate.saturating_sub(1)
            };
            self.pool_hit_rates[pool_index].store(new_rate, Ordering::Relaxed);
        }
    }

    pub fn calculate_efficiency_score(&self, pool_index: usize) -> u32 {
        if pool_index >= 3 {
            return 0;
        }

        let hit_rate = self.pool_hit_rates[pool_index].load(Ordering::Relaxed);
        let utilization = self.pool_utilizations[pool_index].load(Ordering::Relaxed);

        // Efficiency combines hit rate and utilization
        let efficiency = (hit_rate as u64 * utilization as u64) / 10000;
        std::cmp::min(efficiency as u32, 10000)
    }
}
