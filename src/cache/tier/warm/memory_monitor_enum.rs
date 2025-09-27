//! Memory monitor for warm tier cache
//!
//! This module provides the memory monitor implementation using
//! the production-grade MemoryPressureMonitor.

use super::memory_monitor_trait::MemoryMonitor;
use super::monitoring::MemoryPressureMonitor;

/// Memory monitor implementation
#[derive(Debug)]
pub struct MemoryMonitorImpl {
    monitor: MemoryPressureMonitor,
}

impl MemoryMonitorImpl {
    /// Create a new memory monitor
    pub fn new(memory_limit: u64) -> Self {
        Self {
            monitor: MemoryPressureMonitor::new(memory_limit),
        }
    }
}

impl MemoryMonitor for MemoryMonitorImpl {
    /// Record memory allocation
    fn record_allocation(&self, size: usize) {
        self.monitor.record_allocation(size);
    }

    /// Record memory deallocation
    fn record_deallocation(&self, size: usize) {
        self.monitor.record_deallocation(size);
    }

    /// Get current memory pressure level
    fn get_pressure(&self) -> f64 {
        self.monitor.get_pressure()
    }

    /// Check if eviction should be triggered
    fn should_evict(&self) -> bool {
        self.monitor.should_evict()
    }

    /// Update memory usage
    fn update_memory_usage(
        &self,
        new_usage: u64,
    ) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        self.monitor.update_memory_usage(new_usage)
    }

    /// Get current memory usage
    fn get_current_usage(&self) -> u64 {
        self.monitor.get_current_usage()
    }
}
