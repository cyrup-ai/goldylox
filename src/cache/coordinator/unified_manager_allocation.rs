//! Memory allocation methods for UnifiedCacheManager
//!
//! This module provides public allocation methods that delegate to the AllocationManager

use super::unified_manager::UnifiedCacheManager;
use crate::cache::memory::types::MemoryStatistics;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use std::ptr::NonNull;

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> UnifiedCacheManager<K, V>
{
    /// Allocate memory using the internal allocation manager
    #[allow(dead_code)] // Public API - allocate method for memory allocation operations
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        self.allocation_manager.allocate(size)
    }

    /// Deallocate memory using the internal allocation manager
    #[allow(dead_code)] // Public API - deallocate method for memory deallocation operations
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) -> Result<(), CacheOperationError> {
        self.allocation_manager.deallocate(ptr, size)
    }

    /// Get current memory statistics from the allocation manager
    #[allow(dead_code)] // Public API - memory statistics retrieval for monitoring
    pub fn get_memory_stats(&self) -> MemoryStatistics {
        self.allocation_manager.get_memory_stats()
    }

    /// Get memory pool manager reference
    #[allow(dead_code)] // Public API - pool manager access for advanced memory operations
    pub fn pool_manager(&self) -> &crate::cache::memory::pool_manager::MemoryPoolManager {
        self.allocation_manager.pool_manager()
    }

    /// Trigger emergency pool cleanup
    #[allow(dead_code)] // Public API - emergency cleanup for memory pressure situations
    pub fn try_emergency_pool_cleanup(&self) -> Result<bool, CacheOperationError> {
        self.allocation_manager.try_emergency_pool_cleanup()
    }

    /// Get pool cleanup statistics
    #[allow(dead_code)] // Public API - cleanup statistics for monitoring pool health
    pub fn get_pool_cleanup_stats(&self) -> (u64, u64, f64) {
        self.allocation_manager.get_pool_cleanup_stats()
    }

    /// Check if pool cleanup is available
    #[allow(dead_code)] // Public API - cleanup availability check
    pub fn is_pool_cleanup_available(&self) -> bool {
        self.allocation_manager.is_pool_cleanup_available()
    }

    /// Get global allocation statistics
    #[allow(dead_code)] // Public API - global allocation statistics for system monitoring
    pub fn get_global_allocation_stats(&self) -> crate::cache::memory::allocation_manager::GlobalAllocationSnapshot {
        self.allocation_manager.get_global_allocation_stats()
    }

    /// Get global pool statistics
    #[allow(dead_code)] // Public API - global pool statistics for system monitoring
    pub fn get_global_pool_stats(&self) -> crate::cache::memory::allocation_manager::GlobalPoolSnapshot {
        self.allocation_manager.get_global_pool_stats()
    }

    /// Record pool allocation for statistics
    #[allow(dead_code)] // Public API - pool allocation recording for statistics tracking
    pub fn record_pool_allocation(&self, pool_idx: usize, size: u64) {
        self.allocation_manager.record_pool_allocation(pool_idx, size)
    }

    /// Record pool hit rate for statistics
    #[allow(dead_code)] // Public API - pool hit rate recording for performance monitoring
    pub fn record_pool_hit_rate(&self, pool_idx: usize, hit_rate: u32) {
        self.allocation_manager.record_pool_hit_rate(pool_idx, hit_rate)
    }
}
