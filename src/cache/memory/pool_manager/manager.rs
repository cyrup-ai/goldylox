//! Memory pool manager with lock-free allocation and dynamic sizing
//!
//! This module implements the main MemoryPoolManager struct that coordinates
//! multiple memory pools for different object sizes.

// Removed unused import
use super::configuration::PoolConfiguration;
use super::individual_pool::MemoryPool;
use super::statistics::PoolStatistics;
use crate::cache::config::CacheConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Memory pool manager for efficient allocation patterns
#[derive(Debug, Clone)]
pub struct MemoryPoolManager {
    /// Small object pool (< 1KB)
    small_pool: MemoryPool,
    /// Medium object pool (1KB - 64KB)
    medium_pool: MemoryPool,
    /// Large object pool (> 64KB)
    large_pool: MemoryPool,
    /// Pool selection statistics
    pool_stats: PoolStatistics,
    /// Pool configuration parameters
    pool_config: PoolConfiguration,
}

impl MemoryPoolManager {
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let small_pool = MemoryPool::new("small", 1024, 10000)?; // 1KB objects, 10K capacity
        let medium_pool = MemoryPool::new("medium", 65536, 1000)?; // 64KB objects, 1K capacity
        let large_pool = MemoryPool::new("large", 1048576, 100)?; // 1MB objects, 100 capacity
        
        Ok(Self {
            small_pool,
            medium_pool,
            large_pool,
            pool_stats: PoolStatistics::new(),
            pool_config: PoolConfiguration::new(),
        })
    }

    /// Get small pool reference
    pub fn small_pool(&self) -> &MemoryPool {
        &self.small_pool
    }

    /// Get medium pool reference
    pub fn medium_pool(&self) -> &MemoryPool {
        &self.medium_pool
    }

    /// Get large pool reference
    pub fn large_pool(&self) -> &MemoryPool {
        &self.large_pool
    }

    /// Sophisticated emergency cleanup across all pools using existing systems
    pub fn try_emergency_cleanup(&self) -> bool {
        // Directly perform simple cleanup since pools no longer have cleanup managers
        self.simple_fallback_emergency_cleanup()
    }

    /// Simple fallback emergency cleanup (preserves original behavior)
    fn simple_fallback_emergency_cleanup(&self) -> bool {
        let small_cleaned = self.small_pool.try_cleanup();
        let medium_cleaned = self.medium_pool.try_cleanup();
        let large_cleaned = self.large_pool.try_cleanup();

        small_cleaned || medium_cleaned || large_cleaned
    }

    /// Get pool statistics
    pub fn get_pool_stats(&self) -> (u64, u64, u64) {
        (
            self.pool_stats.pool_allocations[0].load(std::sync::atomic::Ordering::Relaxed),
            self.pool_stats.pool_allocations[1].load(std::sync::atomic::Ordering::Relaxed),
            self.pool_stats.pool_allocations[2].load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Update pool utilization metrics
    pub fn update_pool_metrics(&self) {
        for i in 0..3 {
            let pool = match i {
                0 => &self.small_pool,
                1 => &self.medium_pool,
                2 => &self.large_pool,
                _ => unreachable!(),
            };

            let utilization = pool.get_utilization_percentage();
            self.pool_stats.pool_utilizations[i]
                .store(utilization, std::sync::atomic::Ordering::Relaxed);
            // Also update global stats
            use crate::cache::memory::allocation_manager::global_stats;
            global_stats::POOL_STATS.pool_utilizations[i]
                .store(utilization, std::sync::atomic::Ordering::Relaxed);

            let efficiency = self.pool_stats.calculate_efficiency_score(i);
            self.pool_stats.efficiency_scores[i]
                .store(efficiency, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
