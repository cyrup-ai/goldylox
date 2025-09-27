//! Pool configuration parameters and sizing logic
//!
//! This module implements the PoolConfiguration struct that manages pool
//! sizing policies, growth factors, and maintenance scheduling.

/// Pool configuration parameters
#[derive(Debug, Clone)]
pub struct PoolConfiguration {
    /// Pool size limits (objects)
    #[allow(dead_code)]
    // Memory management - size_limits used in pool size configuration management
    size_limits: [usize; 3], // Small, Medium, Large pool sizes
    /// Growth factors when expanding pools
    growth_factors: [f32; 3], // Growth multipliers
    /// Shrink thresholds (utilization percentage)
    shrink_thresholds: [u32; 3], // Percentage * 100
    /// Pool maintenance intervals (nanoseconds)
    maintenance_intervals: [u64; 3],
}

impl PoolConfiguration {
    pub fn new() -> Self {
        Self {
            size_limits: [
                10000, // Small pool: 10K objects
                1000,  // Medium pool: 1K objects
                100,   // Large pool: 100 objects
            ],
            growth_factors: [
                1.5, // Small pool grows by 50%
                1.3, // Medium pool grows by 30%
                1.2, // Large pool grows by 20%
            ],
            shrink_thresholds: [
                3000, // Small pool shrinks below 30% utilization
                2500, // Medium pool shrinks below 25% utilization
                2000, // Large pool shrinks below 20% utilization
            ],
            maintenance_intervals: [
                60_000_000_000,  // Small pool: 60 seconds
                120_000_000_000, // Medium pool: 120 seconds
                300_000_000_000, // Large pool: 300 seconds
            ],
        }
    }

    #[allow(dead_code)] // Memory management - should_grow_pool used in pool growth decision making
    pub fn should_grow_pool(&self, pool_index: usize, current_utilization: u32) -> bool {
        if pool_index >= 3 {
            return false;
        }

        // Grow if utilization exceeds 80%
        current_utilization > 8000
    }

    pub fn should_shrink_pool(&self, pool_index: usize, current_utilization: u32) -> bool {
        if pool_index >= 3 {
            return false;
        }

        // Shrink if utilization falls below configured threshold
        current_utilization < self.shrink_thresholds[pool_index]
    }

    pub fn calculate_new_pool_size(
        &self,
        pool_index: usize,
        current_size: usize,
        should_grow: bool,
    ) -> usize {
        if pool_index >= 3 {
            return current_size;
        }

        if should_grow {
            let growth_factor = self.growth_factors[pool_index];
            (current_size as f32 * growth_factor) as usize
        } else {
            // Shrink by inverse of growth factor
            let shrink_factor = 1.0 / self.growth_factors[pool_index];
            std::cmp::max((current_size as f32 * shrink_factor) as usize, 100) // Minimum 100 objects
        }
    }
}
