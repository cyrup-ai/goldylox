//! Allocation statistics tracking with atomic operations
//!
//! This module provides thread-safe allocation statistics tracking using atomic
//! operations for high-performance concurrent access.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crossbeam_utils::CachePadded;

use super::types::MemoryStatistics;

/// Atomic allocation statistics for performance monitoring
#[derive(Debug)]
pub struct AllocationStatistics {
    /// Total allocated memory (bytes)
    total_allocated: CachePadded<AtomicU64>,
    /// Peak allocation reached (bytes)
    peak_allocation: CachePadded<AtomicU64>,
    /// Current active allocations count
    active_allocations: CachePadded<AtomicUsize>,
    /// Total allocation operations performed
    allocation_operations: CachePadded<AtomicU64>,
    /// Total deallocation operations performed
    deallocation_operations: CachePadded<AtomicU64>,
    /// Failed allocation attempts
    allocation_failures: CachePadded<AtomicU64>,
    /// Memory fragmentation level (percentage * 100)
    fragmentation_level: CachePadded<AtomicU32>,
    /// Average allocation size (bytes)
    #[allow(dead_code)]
    // Memory management - avg_allocation_size used in allocation size statistics
    avg_allocation_size: CachePadded<AtomicUsize>,
}

impl AllocationStatistics {
    /// Create new allocation statistics tracker
    pub fn new() -> Self {
        Self {
            total_allocated: CachePadded::new(AtomicU64::new(0)),
            peak_allocation: CachePadded::new(AtomicU64::new(0)),
            active_allocations: CachePadded::new(AtomicUsize::new(0)),
            allocation_operations: CachePadded::new(AtomicU64::new(0)),
            deallocation_operations: CachePadded::new(AtomicU64::new(0)),
            allocation_failures: CachePadded::new(AtomicU64::new(0)),
            fragmentation_level: CachePadded::new(AtomicU32::new(0)),
            avg_allocation_size: CachePadded::new(AtomicUsize::new(1024)), // 1KB default
        }
    }

    /// Record a memory allocation
    #[allow(dead_code)] // Memory management - record_allocation used in allocation statistics tracking
    pub fn record_allocation(&self, size: u64) {
        self.total_allocated.fetch_add(size, Ordering::Relaxed);
        self.allocation_operations.fetch_add(1, Ordering::Relaxed);
        self.active_allocations.fetch_add(1, Ordering::Relaxed);

        // Update average allocation size
        self.update_average_allocation_size(size);
    }

    /// Record a memory deallocation
    #[allow(dead_code)] // Memory management - deallocation recording used in advanced memory tracking
    pub fn record_deallocation(&self, size: u64) {
        self.total_allocated.fetch_sub(size, Ordering::Relaxed);
        self.deallocation_operations.fetch_add(1, Ordering::Relaxed);
        self.active_allocations.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record an allocation failure  
    #[allow(dead_code)] // Memory management - allocation failure tracking used in advanced memory analytics
    pub fn record_allocation_failure(&self) {
        self.allocation_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Update peak allocation atomically
    #[allow(dead_code)] // Memory management - peak allocation tracking used in memory optimization
    pub fn update_peak_allocation(&self) {
        let current_total = self.total_allocated.load(Ordering::Relaxed);
        let mut current_peak = self.peak_allocation.load(Ordering::Relaxed);

        while current_total > current_peak {
            match self.peak_allocation.compare_exchange_weak(
                current_peak,
                current_total,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_peak = actual,
            }
        }
    }

    /// Update fragmentation level
    #[allow(dead_code)] // Memory management - fragmentation tracking used in memory optimization
    pub fn update_fragmentation_level(&self, fragmentation_percentage: f32) {
        let fragmentation_scaled = (fragmentation_percentage * 100.0) as u32;
        self.fragmentation_level
            .store(fragmentation_scaled, Ordering::Relaxed);
    }

    /// Get current memory statistics snapshot
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    pub fn get_statistics(&self) -> MemoryStatistics {
        MemoryStatistics {
            total_allocated: self.total_allocated.load(Ordering::Relaxed),
            peak_allocation: self.peak_allocation.load(Ordering::Relaxed),
            active_allocations: self.active_allocations.load(Ordering::Relaxed),
            allocation_operations: self.allocation_operations.load(Ordering::Relaxed),
            deallocation_operations: self.deallocation_operations.load(Ordering::Relaxed),
            allocation_failures: self.allocation_failures.load(Ordering::Relaxed),
            fragmentation_level: self.fragmentation_level.load(Ordering::Relaxed) as f32 / 100.0,
            pressure_level: 0.0, // Will be set by pressure monitor
        }
    }

    /// Get total allocated memory
    #[allow(dead_code)] // Memory management - allocation tracking getter used in memory monitoring
    pub fn total_allocated(&self) -> u64 {
        self.total_allocated.load(Ordering::Relaxed)
    }

    /// Get active allocations count
    #[allow(dead_code)] // Memory management - active allocation tracking used in memory monitoring
    pub fn active_allocations(&self) -> usize {
        self.active_allocations.load(Ordering::Relaxed)
    }

    /// Get allocation failure count
    #[allow(dead_code)] // Memory management - failure tracking used in error analysis
    pub fn allocation_failures(&self) -> u64 {
        self.allocation_failures.load(Ordering::Relaxed)
    }

    /// Get fragmentation level
    #[allow(dead_code)] // Memory management - fragmentation level getter used in memory optimization
    pub fn fragmentation_level(&self) -> f32 {
        self.fragmentation_level.load(Ordering::Relaxed) as f32 / 100.0
    }

    /// Update average allocation size using exponential moving average
    fn update_average_allocation_size(&self, new_size: u64) {
        let current_avg = self.avg_allocation_size.load(Ordering::Relaxed);
        // Exponential moving average with alpha = 0.1
        let new_avg = (current_avg * 9 + new_size as usize) / 10;
        self.avg_allocation_size.store(new_avg, Ordering::Relaxed);
    }
}

impl Default for AllocationStatistics {
    fn default() -> Self {
        Self::new()
    }
}
