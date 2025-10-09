//! Types and enums for memory monitoring and alerting
//!
//! This module defines the core data structures used throughout the monitoring system.

#![allow(dead_code)] // Warm tier monitoring - Complete type definitions library for monitoring subsystem

use std::sync::atomic::AtomicU64;

use crossbeam_utils::CachePadded;

use crate::cache::tier::warm::atomic_float::AtomicF64;

/// Memory pressure thresholds
#[derive(Debug, Clone)]
pub struct PressureThresholds {
    /// Low pressure threshold (0.0-1.0)
    pub low_pressure: f64,
    /// Medium pressure threshold
    pub medium_pressure: f64,
    /// High pressure threshold
    pub high_pressure: f64,
    /// Critical pressure threshold
    pub critical_pressure: f64,
}

impl Default for PressureThresholds {
    fn default() -> Self {
        Self {
            low_pressure: 0.5,       // 50%
            medium_pressure: 0.7,    // 70%
            high_pressure: 0.85,     // 85%
            critical_pressure: 0.95, // 95%
        }
    }
}

/// Memory alert notification
#[derive(Debug, Clone)]
pub enum MemoryAlert {
    /// Low pressure warning
    LowPressure {
        current_usage: u64,
        pressure_level: f64,
        timestamp_ns: u64,
    },
    /// Medium pressure warning
    MediumPressure {
        current_usage: u64,
        pressure_level: f64,
        trend_direction: f64,
        timestamp_ns: u64,
    },
    /// High pressure alert
    HighPressure {
        current_usage: u64,
        pressure_level: f64,
        time_to_limit_sec: f64,
        timestamp_ns: u64,
    },
    /// Critical pressure - immediate action required
    CriticalPressure {
        current_usage: u64,
        pressure_level: f64,
        recommended_action: String,
        timestamp_ns: u64,
    },
    /// Memory leak detection
    MemoryLeak {
        growth_rate_mb_per_min: f64,
        duration_min: f64,
        timestamp_ns: u64,
    },
    /// OOM risk warning
    OomRisk {
        risk_score: f64,
        time_to_oom_sec: f64,
        timestamp_ns: u64,
    },
}

// Use the real implementation directly
pub use super::alert_system::MemoryAlertSystem;

/// Memory monitoring statistics
#[derive(Debug)]
pub struct MemoryMonitoringStats {
    /// Total monitoring updates
    pub monitoring_updates: CachePadded<AtomicU64>,
    /// Memory allocation events
    pub allocation_events: CachePadded<AtomicU64>,
    /// Memory deallocation events
    pub deallocation_events: CachePadded<AtomicU64>,
    /// Peak pressure level reached
    pub peak_pressure_level: AtomicF64,
    /// Time spent in high pressure
    pub high_pressure_time_ms: CachePadded<AtomicU64>,
    /// OOM (Out of Memory) near-misses
    pub oom_near_misses: CachePadded<AtomicU64>,
}

impl Default for MemoryMonitoringStats {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryMonitoringStats {
    pub fn new() -> Self {
        Self {
            monitoring_updates: CachePadded::new(AtomicU64::new(0)),
            allocation_events: CachePadded::new(AtomicU64::new(0)),
            deallocation_events: CachePadded::new(AtomicU64::new(0)),
            peak_pressure_level: AtomicF64::new(0.0),
            high_pressure_time_ms: CachePadded::new(AtomicU64::new(0)),
            oom_near_misses: CachePadded::new(AtomicU64::new(0)),
        }
    }
}

/// Performance monitoring task for background analysis
#[derive(Debug)]
pub enum MonitoringTask {
    /// Update performance metrics
    UpdateMetrics,
    /// Analyze memory trends
    AnalyzeMemoryTrends,
    /// Generate performance report
    GenerateReport {
        include_history: bool,
        include_predictions: bool,
    },
    /// Check for anomalies
    AnomalyDetection,
    /// Cleanup old monitoring data
    CleanupOldData { max_age_sec: u64 },
}

/// Snapshot of tier statistics at a point in time
#[derive(Debug, Clone)]
pub struct TierStatsSnapshot {
    pub entry_count: usize,
    pub memory_usage: u64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub hit_rate: f64,
    pub avg_access_latency_ns: f64,
    pub peak_access_latency_ns: u64,
    pub ops_per_second: f64,
    pub performance_score: f64,
    // Enhanced fields for comprehensive monitoring
    pub timestamp_ns: u64,
    pub memory_pressure: f64,
    pub evictions: u64,
}

impl From<crate::cache::types::statistics::tier_stats::TierStatistics> for TierStatsSnapshot {
    fn from(stats: crate::cache::types::statistics::tier_stats::TierStatistics) -> Self {
        Self {
            entry_count: stats.entry_count,
            memory_usage: stats.memory_usage as u64,
            total_hits: stats.hits,
            total_misses: stats.misses,
            hit_rate: stats.hit_rate,
            avg_access_latency_ns: stats.avg_access_time_ns as f64,
            peak_access_latency_ns: stats.avg_access_time_ns,
            ops_per_second: stats.ops_per_second,
            performance_score: stats.efficiency_score(),
            timestamp_ns: crate::cache::types::timestamp_nanos(std::time::Instant::now()),
            memory_pressure: 0.0,
            evictions: 0,
        }
    }
}

impl Default for TierStatsSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

impl TierStatsSnapshot {
    /// Create new snapshot with current timestamp
    pub fn new() -> Self {
        Self {
            entry_count: 0,
            memory_usage: 0,
            total_hits: 0,
            total_misses: 0,
            hit_rate: 0.0,
            avg_access_latency_ns: 0.0,
            peak_access_latency_ns: 0,
            ops_per_second: 0.0,
            performance_score: 0.0,
            timestamp_ns: crate::cache::types::timestamp_nanos(std::time::Instant::now()),
            memory_pressure: 0.0,
            evictions: 0,
        }
    }

    /// Calculate efficiency ratio
    pub fn efficiency_ratio(&self) -> f64 {
        if self.total_hits + self.total_misses == 0 {
            0.0
        } else {
            self.hit_rate * (1.0 - self.memory_pressure)
        }
    }
}
