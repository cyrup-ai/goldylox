//! Memory monitor enum for interchangeable implementations
//!
//! This module provides an enum that can hold either a real memory monitor
//! or a no-op monitor for graceful degradation.

use super::memory_monitor_trait::MemoryMonitor;
use super::monitoring::MemoryPressureMonitor;
use super::noop_monitor::NoOpMemoryMonitor;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Enum for memory monitor implementations that supports graceful degradation
#[derive(Debug)]
pub enum MemoryMonitorImpl {
    /// Real memory pressure monitor
    Real(MemoryPressureMonitor),
    /// No-op monitor for graceful degradation
    NoOp(NoOpMemoryMonitor),
}

impl MemoryMonitorImpl {
    /// Create a new real memory monitor
    pub fn new_real(memory_limit: u64) -> Result<Self, CacheOperationError> {
        Ok(Self::Real(MemoryPressureMonitor::new(memory_limit)?))
    }

    /// Create a new no-op monitor
    pub fn new_noop() -> Self {
        Self::NoOp(NoOpMemoryMonitor::new())
    }
}

impl MemoryMonitor for MemoryMonitorImpl {
    /// Record memory allocation
    fn record_allocation(&self, size: usize) {
        match self {
            Self::Real(monitor) => monitor.record_allocation(size),
            Self::NoOp(monitor) => monitor.record_allocation(size),
        }
    }

    /// Record memory deallocation
    fn record_deallocation(&self, size: usize) {
        match self {
            Self::Real(monitor) => monitor.record_deallocation(size),
            Self::NoOp(monitor) => monitor.record_deallocation(size),
        }
    }

    /// Get current memory pressure level
    fn get_pressure(&self) -> f64 {
        match self {
            Self::Real(monitor) => monitor.get_pressure(),
            Self::NoOp(monitor) => monitor.get_pressure(),
        }
    }

    /// Check if eviction should be triggered
    fn should_evict(&self) -> bool {
        match self {
            Self::Real(monitor) => monitor.should_evict(),
            Self::NoOp(monitor) => monitor.should_evict(),
        }
    }

    /// Update memory usage
    fn update_memory_usage(&self, new_usage: u64) -> Result<(), CacheOperationError> {
        match self {
            Self::Real(monitor) => monitor.update_memory_usage(new_usage),
            Self::NoOp(monitor) => monitor.update_memory_usage(new_usage),
        }
    }

    /// Get current memory usage
    fn get_current_usage(&self) -> u64 {
        match self {
            Self::Real(monitor) => monitor.get_current_usage(),
            Self::NoOp(monitor) => monitor.get_current_usage(),
        }
    }
}

// REMOVED: get_pressure_level() compatibility alias
// Users must now use canonical method:
// - Use get_pressure() instead of get_pressure_level()
