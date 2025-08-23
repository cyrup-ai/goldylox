//! No-op memory monitor for graceful degradation
//!
//! This module provides a stub memory monitor implementation that can be used
//! when the real memory monitor fails to initialize, allowing the cache to
//! operate in a degraded mode without memory pressure monitoring.

/// No-op memory monitor for graceful degradation
#[derive(Debug)]
pub struct NoOpMemoryMonitor;

impl NoOpMemoryMonitor {
    /// Create new no-op memory monitor
    pub fn new() -> Self {
        Self
    }

    /// Record memory allocation (no-op)
    pub fn record_allocation(&self, _size: usize) {
        // No-op: do nothing
    }

    /// Record memory deallocation (no-op)
    pub fn record_deallocation(&self, _size: usize) {
        // No-op: do nothing
    }

    /// Get current memory pressure (always returns 0.0)
    pub fn get_pressure(&self) -> f64 {
        0.0
    }

    /// Check if eviction should be triggered (always returns false)
    pub fn should_evict(&self) -> bool {
        false
    }

    /// Update memory usage (no-op)
    pub fn update_memory_usage(
        &self,
        _new_usage: u64,
    ) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        Ok(())
    }

    /// Get current memory usage (always returns 0)
    pub fn get_current_usage(&self) -> u64 {
        0
    }

    /// Get memory pressure level (always returns 0.0)
    pub fn get_pressure_level(&self) -> f64 {
        0.0
    }
}

impl Default for NoOpMemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl super::memory_monitor_trait::MemoryMonitor for NoOpMemoryMonitor {
    fn record_allocation(&self, _size: usize) {
        // No-op: do nothing
    }

    fn record_deallocation(&self, _size: usize) {
        // No-op: do nothing
    }

    fn get_pressure(&self) -> f64 {
        0.0
    }

    fn should_evict(&self) -> bool {
        false
    }

    fn update_memory_usage(
        &self,
        _new_usage: u64,
    ) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        Ok(())
    }

    fn get_current_usage(&self) -> u64 {
        0
    }
}
