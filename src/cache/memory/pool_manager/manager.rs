//! Memory pool manager with lock-free allocation and dynamic sizing
//!
//! This module implements the main MemoryPoolManager struct that coordinates
//! multiple memory pools for different object sizes.

use std::sync::Arc;

use super::cleanup_manager::PoolCleanupManager;
use super::configuration::PoolConfiguration;
use super::individual_pool::MemoryPool;
use super::statistics::PoolStatistics;
use crate::cache::config::CacheConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Memory pool manager for efficient allocation patterns
#[derive(Debug)]
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
    /// Sophisticated cleanup manager (integrates with existing systems)
    cleanup_manager: Option<Arc<PoolCleanupManager>>,
}

impl MemoryPoolManager {
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        Self::new_with_cleanup(_config, None)
    }

    /// Create new memory pool manager with optional cleanup manager injection
    pub fn new_with_cleanup(
        _config: &CacheConfig,
        cleanup_manager: Option<Arc<PoolCleanupManager>>,
    ) -> Result<Self, CacheOperationError> {
        let mut small_pool = MemoryPool::new("small", 1024, 10000)?; // 1KB objects, 10K capacity
        let mut medium_pool = MemoryPool::new("medium", 65536, 1000)?; // 64KB objects, 1K capacity
        let mut large_pool = MemoryPool::new("large", 1048576, 100)?; // 1MB objects, 100 capacity
        
        // Inject cleanup manager into all pools if provided
        if let Some(ref cleanup_mgr) = cleanup_manager {
            small_pool.set_cleanup_manager(cleanup_mgr.clone());
            medium_pool.set_cleanup_manager(cleanup_mgr.clone());
            large_pool.set_cleanup_manager(cleanup_mgr.clone());
        }
        
        Ok(Self {
            small_pool,
            medium_pool,
            large_pool,
            pool_stats: PoolStatistics::new(),
            pool_config: PoolConfiguration::new(),
            cleanup_manager,
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

    /// Set the cleanup manager for sophisticated cleanup operations
    pub fn set_cleanup_manager(&mut self, cleanup_manager: Arc<PoolCleanupManager>) {
        self.cleanup_manager = Some(cleanup_manager);
    }

    /// Sophisticated emergency cleanup across all pools using existing systems
    pub fn try_emergency_cleanup(&self) -> bool {
        // Use sophisticated cleanup manager if available
        if let Some(ref cleanup_manager) = self.cleanup_manager {
            // Coordinate emergency cleanup across all pools using existing systems
            let pools = [&self.small_pool, &self.medium_pool, &self.large_pool];
            match cleanup_manager.try_emergency_cleanup_coordination(&pools) {
                Ok(cleanup_performed) => cleanup_performed,
                Err(_) => {
                    // Fallback to simple cleanup if sophisticated cleanup fails
                    self.simple_fallback_emergency_cleanup()
                }
            }
        } else {
            // Fallback to simple cleanup if no cleanup manager
            self.simple_fallback_emergency_cleanup()
        }
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

            let efficiency = self.pool_stats.calculate_efficiency_score(i);
            self.pool_stats.efficiency_scores[i]
                .store(efficiency, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
