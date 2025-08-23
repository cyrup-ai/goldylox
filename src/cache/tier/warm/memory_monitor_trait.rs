//! Memory monitor trait for interchangeable monitor implementations
//!
//! This module defines a trait that allows both real memory monitors and
//! no-op monitors to be used interchangeably in the warm tier cache.

use crate::cache::traits::types_and_enums::CacheOperationError;

/// Trait for memory monitoring implementations
pub trait MemoryMonitor: std::fmt::Debug + Send + Sync {
    /// Record memory allocation
    fn record_allocation(&self, size: usize);

    /// Record memory deallocation  
    fn record_deallocation(&self, size: usize);

    /// Get current memory pressure level (0.0-1.0)
    fn get_pressure(&self) -> f64;

    /// Check if eviction should be triggered
    fn should_evict(&self) -> bool;

    /// Update memory usage
    fn update_memory_usage(&self, new_usage: u64) -> Result<(), CacheOperationError>;

    /// Get current memory usage in bytes
    fn get_current_usage(&self) -> u64;

    // REMOVED: get_pressure_level() compatibility alias
    // Users must now use canonical method:
    // - Use get_pressure() instead of get_pressure_level()
}
