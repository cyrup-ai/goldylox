//! Memory pressure and performance monitoring for warm tier cache
//!
//! This module implements comprehensive monitoring and alerting for cache performance,
//! memory usage, and system health with atomic counters and lock-free data collection.
//!
//! The monitoring system is decomposed into focused submodules:
//! - `memory_pressure`: Core memory pressure monitoring and alerting
//! - `usage_history`: Historical tracking and trend analysis  
//! - `alert_system`: Alert generation and notification management
//! - `performance_stats`: Atomic tier statistics and performance tracking
//! - `trend_analysis`: Memory usage pattern analysis
//! - `types`: Configuration types and alert definitions

#![allow(dead_code)] // Warm tier monitoring - Complete monitoring and alerting library for memory pressure, performance stats, and trend analysis

pub mod alert_system;
pub mod memory_pressure;
pub mod performance_stats;
pub mod trend_analysis;
pub mod types;
pub mod usage_history;

use types::MemoryAlertSystem;
use usage_history::MemoryUsageHistory;

// Re-export main types
pub use crate::cache::types::statistics::atomic_stats::AtomicTierStats;
pub use memory_pressure::MemoryPressureMonitor;
pub use types::TierStatsSnapshot;

/// Create new memory pressure monitor with default settings
pub fn create_memory_monitor(memory_limit: u64) -> MemoryPressureMonitor {
    MemoryPressureMonitor::new(memory_limit)
}

/// Create new atomic tier statistics
pub fn create_tier_stats() -> AtomicTierStats {
    AtomicTierStats::new()
}

/// Create new memory usage history tracker
pub fn create_usage_history() -> MemoryUsageHistory {
    MemoryUsageHistory::new()
}

/// Create new alert system with specified cooldown
pub fn create_alert_system(cooldown_ms: u64) -> MemoryAlertSystem {
    MemoryAlertSystem::new(cooldown_ms)
}

/// Utility function to check if pressure level is critical
pub fn is_critical_pressure(pressure: f64) -> bool {
    pressure >= 0.95
}

/// Utility function to check if pressure level is high
pub fn is_high_pressure(pressure: f64) -> bool {
    pressure >= 0.85
}

/// Utility function to check if pressure level is medium
pub fn is_medium_pressure(pressure: f64) -> bool {
    pressure >= 0.7
}

/// Utility function to check if pressure level is low
pub fn is_low_pressure(pressure: f64) -> bool {
    pressure >= 0.5
}

/// Calculate memory efficiency score based on usage and hit rate
pub fn calculate_efficiency_score(memory_usage: u64, memory_limit: u64, hit_rate: f64) -> f64 {
    let usage_efficiency = 1.0 - (memory_usage as f64 / memory_limit as f64);
    let performance_weight = 0.7;
    let memory_weight = 0.3;

    (hit_rate * performance_weight) + (usage_efficiency * memory_weight)
}

/// Format memory size for human-readable display
pub fn format_memory_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Calculate estimated time to OOM based on current trend
pub fn estimate_oom_time(
    current_usage: u64,
    memory_limit: u64,
    change_rate_bytes_per_sec: f64,
) -> Option<f64> {
    if change_rate_bytes_per_sec <= 0.0 || current_usage >= memory_limit {
        return None;
    }

    let remaining_bytes = memory_limit.saturating_sub(current_usage);
    Some(remaining_bytes as f64 / change_rate_bytes_per_sec)
}
