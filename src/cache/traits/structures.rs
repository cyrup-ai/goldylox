//! Cache data structures for performance tracking and capacity management
//!
//! This module provides blazing-fast data structures optimized for zero-allocation
//! cache performance monitoring and management operations.

use std::fmt::Debug;
use std::time::Duration;

/// Capacity information structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityInfo {
    /// Current entry count
    pub current: usize,
    /// Maximum entries
    pub maximum: usize,
    /// Memory usage in bytes
    pub memory_bytes: usize,
    /// Memory limit in bytes
    pub memory_limit: usize,
}

impl CapacityInfo {
    /// Create new capacity info
    #[inline(always)]
    pub const fn new(
        current: usize,
        maximum: usize,
        memory_bytes: usize,
        memory_limit: usize,
    ) -> Self {
        Self {
            current,
            maximum,
            memory_bytes,
            memory_limit,
        }
    }

    /// Get utilization percentage (0.0-1.0)
    #[inline(always)]
    pub fn utilization(&self) -> f64 {
        if self.maximum > 0 {
            self.current as f64 / self.maximum as f64
        } else {
            0.0
        }
    }

    /// Get memory utilization percentage (0.0-1.0)
    #[inline(always)]
    pub fn memory_utilization(&self) -> f64 {
        if self.memory_limit > 0 {
            self.memory_bytes as f64 / self.memory_limit as f64
        } else {
            0.0
        }
    }

    /// Check if at capacity
    #[inline(always)]
    pub fn is_at_capacity(&self) -> bool {
        self.current >= self.maximum || self.memory_bytes >= self.memory_limit
    }

    /// Check if under memory pressure
    #[inline(always)]
    pub fn is_under_pressure(&self) -> bool {
        self.memory_utilization() > 0.8 || self.utilization() > 0.9
    }

    /// Get available capacity
    #[inline(always)]
    pub fn available_capacity(&self) -> usize {
        self.maximum.saturating_sub(self.current)
    }

    /// Get available memory
    #[inline(always)]
    pub fn available_memory(&self) -> usize {
        self.memory_limit.saturating_sub(self.memory_bytes)
    }
}

/// Maintenance report structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceReport {
    /// Entries cleaned up
    pub cleaned_entries: usize,
    /// Bytes reclaimed
    pub bytes_reclaimed: usize,
    /// Time spent in maintenance
    pub duration: Duration,
    /// Issues found and fixed
    pub issues_resolved: usize,
}

impl MaintenanceReport {
    /// Create new maintenance report
    #[inline(always)]
    pub const fn new(
        cleaned_entries: usize,
        bytes_reclaimed: usize,
        duration: Duration,
        issues_resolved: usize,
    ) -> Self {
        Self {
            cleaned_entries,
            bytes_reclaimed,
            duration,
            issues_resolved,
        }
    }

    /// Check if maintenance was effective
    #[inline(always)]
    pub fn was_effective(&self) -> bool {
        self.cleaned_entries > 0 || self.bytes_reclaimed > 0 || self.issues_resolved > 0
    }

    /// Get cleanup rate (entries per second)
    #[inline(always)]
    pub fn cleanup_rate(&self) -> f64 {
        let duration_secs = self.duration.as_secs_f64();
        if duration_secs > 0.0 {
            self.cleaned_entries as f64 / duration_secs
        } else {
            0.0
        }
    }

    /// Get data reclaim rate (bytes per second)
    #[inline(always)]
    pub fn reclaim_rate(&self) -> f64 {
        let duration_secs = self.duration.as_secs_f64();
        if duration_secs > 0.0 {
            self.bytes_reclaimed as f64 / duration_secs
        } else {
            0.0
        }
    }

    /// Get issue resolution efficiency
    #[inline(always)]
    pub fn resolution_efficiency(&self) -> f64 {
        if self.cleaned_entries > 0 {
            self.issues_resolved as f64 / self.cleaned_entries as f64
        } else {
            0.0
        }
    }
}

/// Performance metrics for policy optimization
#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceMetrics {
    /// Overall hit rate
    pub hit_rate: f64,
    /// Average access time
    pub avg_access_time_ns: u64,
    /// Operations per second
    pub ops_per_second: f64,
    /// Memory efficiency ratio
    pub memory_efficiency: f64,
}

impl PerformanceMetrics {
    /// Create new performance metrics
    #[inline(always)]
    pub const fn new(
        hit_rate: f64,
        avg_access_time_ns: u64,
        ops_per_second: f64,
        memory_efficiency: f64,
    ) -> Self {
        Self {
            hit_rate,
            avg_access_time_ns,
            ops_per_second,
            memory_efficiency,
        }
    }

    /// Calculate overall performance score (0.0-1.0)
    #[inline(always)]
    pub fn performance_score(&self) -> f64 {
        let hit_rate_score = self.hit_rate;
        let latency_score = 1.0 / (1.0 + (self.avg_access_time_ns as f64 / 1_000_000.0));
        let throughput_score = (self.ops_per_second / 10000.0).min(1.0);
        let efficiency_score = self.memory_efficiency;

        (hit_rate_score * 0.3)
            + (latency_score * 0.3)
            + (throughput_score * 0.2)
            + (efficiency_score * 0.2)
    }

    /// Check if performance is optimal
    #[inline(always)]
    pub fn is_optimal(&self) -> bool {
        self.performance_score() > 0.8
    }

    /// Check if performance needs attention
    #[inline(always)]
    pub fn needs_attention(&self) -> bool {
        self.performance_score() < 0.5 || self.hit_rate < 0.3
    }

    /// Get latency category
    #[inline(always)]
    pub fn latency_category(&self) -> LatencyCategory {
        match self.avg_access_time_ns {
            0..=100_000 => LatencyCategory::Excellent,       // < 100μs
            100_001..=1_000_000 => LatencyCategory::Good,    // 100μs - 1ms
            1_000_001..=10_000_000 => LatencyCategory::Fair, // 1ms - 10ms
            _ => LatencyCategory::Poor,                      // > 10ms
        }
    }

    /// Get throughput category
    #[inline(always)]
    pub fn throughput_category(&self) -> ThroughputCategory {
        match self.ops_per_second as u64 {
            0..=100 => ThroughputCategory::Low,
            101..=1_000 => ThroughputCategory::Medium,
            1_001..=10_000 => ThroughputCategory::High,
            _ => ThroughputCategory::VeryHigh,
        }
    }
}

/// Latency performance categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatencyCategory {
    Excellent,
    Good,
    Fair,
    Poor,
}

/// Throughput performance categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThroughputCategory {
    Low,
    Medium,
    High,
    VeryHigh,
}
