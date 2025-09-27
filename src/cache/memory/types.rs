#![allow(dead_code)]
// Memory management types - Complete memory management library with allocation statistics, region management, and size distribution tracking

//! Common types and data structures for memory management
//!
//! This module contains shared types used across the memory management subsystem.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Error types for memory detection operations
#[allow(dead_code)] // Memory management - MemoryError used in memory detection error handling
#[derive(Debug)]
pub enum MemoryError {
    SystemCallFailed(String),
    ParseError,
    MemInfoParseError,
    UnsupportedPlatform,
}

/// Memory statistics snapshot
#[allow(dead_code)] // Memory management - MemoryStatistics used in memory statistics reporting
#[derive(Debug, Clone)]
pub struct MemoryStatistics {
    pub total_allocated: u64,
    pub peak_allocation: u64,
    pub active_allocations: usize,
    pub allocation_operations: u64,
    pub deallocation_operations: u64,
    pub allocation_failures: u64,
    pub fragmentation_level: f32,
    pub pressure_level: f32,
}

/// Efficiency analysis result with recommendations
#[allow(dead_code)] // Memory management - EfficiencyAnalysisResult used in efficiency analysis reporting
#[derive(Debug, Clone)]
pub struct EfficiencyAnalysisResult {
    pub timestamp: u64,
    pub efficiency_score: f32,
    pub average_latency_ns: u64,
    pub fragmentation_impact: f32,
    pub memory_utilization: f32,
    pub recommendations: Vec<OptimizationRecommendation>,
}

/// Optimization recommendations for memory management
#[allow(dead_code)] // Memory management - OptimizationRecommendation used in memory optimization suggestions
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationRecommendation {
    /// Increase memory pool sizes
    IncreasePoolSizes,
    /// Trigger memory defragmentation
    TriggerDefragmentation,
    /// Optimize allocation code paths
    OptimizeAllocationPaths,
    /// No action needed
    NoActionNeeded,
}

/// Garbage collection task types
#[allow(dead_code)] // Memory management - GCTaskType used in garbage collection task classification
#[derive(Debug, Clone, Copy)]
pub enum GCTaskType {
    Normal,
    Emergency,
    Maintenance,
}

/// Garbage collection task
#[allow(dead_code)] // Memory management - GCTask used in garbage collection task management
#[derive(Debug, Clone)]
pub struct GCTask {
    pub task_type: GCTaskType,
    pub priority: u8,
    pub scheduled_time: u64,     // Nanoseconds since epoch
    pub estimated_duration: u64, // Nanoseconds
}

impl Default for GCTask {
    fn default() -> Self {
        Self {
            task_type: GCTaskType::Normal,
            priority: 5,
            scheduled_time: 0,
            estimated_duration: 10_000_000, // 10ms default
        }
    }
}

/// Memory pressure sample for trend analysis
#[allow(dead_code)] // Memory management - PressureSample used in memory pressure trend analysis
#[derive(Debug, Clone, Copy)]
pub struct PressureSample {
    pub timestamp: u64,        // Nanoseconds since epoch
    pub pressure_level: u32,   // Percentage * 100
    pub available_memory: u64, // Bytes
    pub allocation_rate: u32,  // Allocations per second
}

/// Analysis result for efficiency tracking
#[allow(dead_code)] // Memory management - AnalysisResult used in efficiency analysis result tracking
#[derive(Debug, Clone, Copy)]
pub struct AnalysisResult {
    pub timestamp: u64,              // Nanoseconds since epoch
    pub utilization_efficiency: u32, // Percentage * 100
    pub allocation_speed: u32,       // Allocations per microsecond * 100
    pub fragmentation_impact: u32,   // Impact percentage * 100
    pub recommendation_count: u32,
}

impl Default for AnalysisResult {
    fn default() -> Self {
        Self {
            timestamp: 0,
            utilization_efficiency: 8500, // 85% default
            allocation_speed: 100000,     // 1000 allocs/μs * 100
            fragmentation_impact: 1500,   // 15% impact
            recommendation_count: 0,
        }
    }
}

/// Memory region for spatial analysis
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start_address: usize,
    pub size: usize,
    pub allocation_count: u32,
    pub last_access_time: u64, // Nanoseconds since epoch
}

/// Pool allocation statistics
#[derive(Debug)]
pub struct PoolAllocationStats {
    pub total_allocations: AtomicU64,
    pub active_allocations: AtomicUsize,
    pub peak_allocations: AtomicUsize,
    pub allocation_failures: AtomicU64,
    pub avg_lifetime: AtomicU64,
}

impl PoolAllocationStats {
    pub fn new() -> Self {
        Self {
            total_allocations: AtomicU64::new(0),
            active_allocations: AtomicUsize::new(0),
            peak_allocations: AtomicUsize::new(0),
            allocation_failures: AtomicU64::new(0),
            avg_lifetime: AtomicU64::new(1_000_000_000), // 1 second default
        }
    }

    pub fn record_allocation(&self) {
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        let current_active = self.active_allocations.fetch_add(1, Ordering::Relaxed) + 1;

        // Update peak if necessary
        let mut current_peak = self.peak_allocations.load(Ordering::Relaxed);
        while current_active > current_peak {
            match self.peak_allocations.compare_exchange_weak(
                current_peak,
                current_active,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_peak = actual,
            }
        }
    }

    pub fn record_deallocation(&self) {
        self.active_allocations.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.allocation_failures.fetch_add(1, Ordering::Relaxed);
    }
}

/// Allocation size distribution for pattern analysis
#[derive(Debug)]
pub struct AllocationSizeDistribution {
    pub small_allocations: AtomicU64,  // < 1KB
    pub medium_allocations: AtomicU64, // 1KB - 64KB
    pub large_allocations: AtomicU64,  // > 64KB
    pub total_size_allocated: AtomicU64,
}

impl Default for AllocationSizeDistribution {
    fn default() -> Self {
        Self {
            small_allocations: AtomicU64::new(0),
            medium_allocations: AtomicU64::new(0),
            large_allocations: AtomicU64::new(0),
            total_size_allocated: AtomicU64::new(0),
        }
    }
}
