//! Memory pressure monitoring with adaptive thresholds
//!
//! This module implements the core memory pressure monitoring logic with
//! real-time alerting and trend analysis.

#![allow(dead_code)] // Warm tier monitoring - Complete memory pressure monitoring library with adaptive thresholds

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crossbeam_utils::CachePadded;

use super::alert_system::MemoryAlertSystem;
use super::types::{MemoryAlert, MemoryMonitoringStats, PressureThresholds};
use super::usage_history::MemoryUsageHistory;
use crate::cache::tier::warm::atomic_float::AtomicF64;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::types::timestamp_nanos;

/// Memory pressure monitor with adaptive thresholds
#[derive(Debug)]
pub struct MemoryPressureMonitor {
    /// Current memory usage in bytes
    current_usage: CachePadded<AtomicU64>,
    /// Memory usage limit in bytes
    memory_limit: u64,
    /// Peak memory usage observed
    peak_usage: CachePadded<AtomicU64>,
    /// Memory pressure level (0.0-1.0)
    pressure_level: AtomicF64,
    /// Memory usage history for trend analysis
    usage_history: MemoryUsageHistory,
    /// Pressure thresholds
    thresholds: PressureThresholds,
    /// Alert system
    alert_system: MemoryAlertSystem,
    /// Monitoring statistics
    stats: MemoryMonitoringStats,
}

impl MemoryPressureMonitor {
    pub fn new(memory_limit: u64) -> Self {
        Self {
            current_usage: CachePadded::new(AtomicU64::new(0)),
            memory_limit,
            peak_usage: CachePadded::new(AtomicU64::new(0)),
            pressure_level: AtomicF64::new(0.0),
            usage_history: MemoryUsageHistory::new(),
            thresholds: PressureThresholds::default(),
            alert_system: MemoryAlertSystem::new(30000), // 30 seconds cooldown
            stats: MemoryMonitoringStats::new(),
        }
    }

    /// Create monitor with custom thresholds
    pub fn with_thresholds(
        memory_limit: u64,
        thresholds: PressureThresholds,
        alert_cooldown_ms: u64,
        max_history_samples: usize,
        _leak_detection_active: bool,
    ) -> Self {
        Self {
            current_usage: CachePadded::new(AtomicU64::new(0)),
            memory_limit,
            peak_usage: CachePadded::new(AtomicU64::new(0)),
            pressure_level: AtomicF64::new(0.0),
            usage_history: MemoryUsageHistory::new_with_capacity(max_history_samples, true),
            thresholds,
            alert_system: MemoryAlertSystem::new(alert_cooldown_ms),
            stats: MemoryMonitoringStats::new(),
        }
    }

    /// Update current memory usage and check pressure levels
    pub fn update_memory_usage(&self, new_usage: u64) -> Result<(), CacheOperationError> {
        let old_usage = self.current_usage.swap(new_usage, Ordering::Relaxed);
        self.stats
            .monitoring_updates
            .fetch_add(1, Ordering::Relaxed);

        // Update peak usage
        self.peak_usage.fetch_max(new_usage, Ordering::Relaxed);

        // Calculate pressure level
        let pressure = new_usage as f64 / self.memory_limit as f64;
        self.pressure_level.store(pressure, Ordering::Relaxed);

        // Update peak pressure
        let current_peak = self.stats.peak_pressure_level.load(Ordering::Relaxed);
        if pressure > current_peak {
            self.stats
                .peak_pressure_level
                .store(pressure, Ordering::Relaxed);
        }

        // Record in history
        self.usage_history
            .record_usage(new_usage, timestamp_nanos(Instant::now()));

        // Check for pressure level changes and alerts
        self.check_pressure_alerts(pressure, new_usage)?;

        // Update trend analysis
        let _ = self.usage_history.update_trend_analysis();

        // Track allocation/deallocation events
        if new_usage > old_usage {
            self.stats.allocation_events.fetch_add(1, Ordering::Relaxed);
        } else if new_usage < old_usage {
            self.stats
                .deallocation_events
                .fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Check pressure levels and send alerts if necessary
    fn check_pressure_alerts(
        &self,
        pressure: f64,
        current_usage: u64,
    ) -> Result<(), CacheOperationError> {
        let current_time = timestamp_nanos(Instant::now());

        let alert = if pressure >= self.thresholds.critical_pressure {
            Some(MemoryAlert::CriticalPressure {
                current_usage,
                pressure_level: pressure,
                recommended_action: "Immediate eviction required".to_string(),
                timestamp_ns: current_time,
            })
        } else if pressure >= self.thresholds.high_pressure {
            let time_to_limit = self.estimate_time_to_limit();
            Some(MemoryAlert::HighPressure {
                current_usage,
                pressure_level: pressure,
                time_to_limit_sec: time_to_limit,
                timestamp_ns: current_time,
            })
        } else if pressure >= self.thresholds.medium_pressure {
            let trend = self
                .usage_history
                .get_trend_analysis()
                .trend_direction
                .load(Ordering::Relaxed);
            Some(MemoryAlert::MediumPressure {
                current_usage,
                pressure_level: pressure,
                trend_direction: trend,
                timestamp_ns: current_time,
            })
        } else if pressure >= self.thresholds.low_pressure {
            Some(MemoryAlert::LowPressure {
                current_usage,
                pressure_level: pressure,
                timestamp_ns: current_time,
            })
        } else {
            None
        };

        // Send alert if generated
        if let Some(alert) = alert {
            match self.alert_system.try_send_alert(alert) {
                Ok(_) => {}
                Err(_) => {
                    // Alert channel full or error - record as near-miss
                    self.stats.oom_near_misses.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        Ok(())
    }

    /// Estimate time until memory limit is reached
    fn estimate_time_to_limit(&self) -> f64 {
        let change_rate = self
            .usage_history
            .get_trend_analysis()
            .change_rate
            .load(Ordering::Relaxed);
        let current_usage = self.current_usage.load(Ordering::Relaxed);
        let remaining_bytes = self.memory_limit.saturating_sub(current_usage);

        if change_rate > 0.0 {
            remaining_bytes as f64 / change_rate
        } else {
            f64::INFINITY // Not growing or shrinking
        }
    }

    /// Get current memory pressure level
    pub fn get_pressure_level(&self) -> f64 {
        self.pressure_level.load(Ordering::Relaxed)
    }

    /// Get current memory usage
    pub fn get_current_usage(&self) -> u64 {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Get memory monitoring statistics
    pub fn get_monitoring_stats(&self) -> (u64, u64, u64, f64, u64, u64) {
        (
            self.stats.monitoring_updates.load(Ordering::Relaxed),
            self.stats.allocation_events.load(Ordering::Relaxed),
            self.stats.deallocation_events.load(Ordering::Relaxed),
            self.stats.peak_pressure_level.load(Ordering::Relaxed),
            self.stats.high_pressure_time_ms.load(Ordering::Relaxed),
            self.stats.oom_near_misses.load(Ordering::Relaxed),
        )
    }

    /// Receive memory alerts (non-blocking)
    pub fn try_receive_alert(&self) -> Option<MemoryAlert> {
        self.alert_system.try_receive_alert()
    }

    /// Check for memory leak patterns
    pub fn detect_memory_leaks(&self) -> Option<MemoryAlert> {
        let trend = self.usage_history.get_trend_analysis();

        let change_rate = trend.change_rate.load(Ordering::Relaxed);
        let trend_strength = trend.trend_strength.load(Ordering::Relaxed);
        let trend_direction = trend.trend_direction.load(Ordering::Relaxed);

        // Detect sustained memory growth
        if trend_direction > 0.8 && trend_strength > 0.7 && change_rate > 0.0 {
            let growth_rate_mb_per_min = (change_rate * 60.0) / (1024.0 * 1024.0);

            // Consider it a leak if growing faster than 10 MB/min for extended period
            if growth_rate_mb_per_min > 10.0 {
                return Some(MemoryAlert::MemoryLeak {
                    growth_rate_mb_per_min,
                    duration_min: 5.0, // Simplified - would track actual duration
                    timestamp_ns: timestamp_nanos(Instant::now()),
                });
            }
        }

        None
    }

    /// Get usage history for external analysis
    pub fn get_usage_history(&self) -> &MemoryUsageHistory {
        &self.usage_history
    }

    /// Update pressure thresholds
    pub fn update_thresholds(&mut self, thresholds: PressureThresholds) {
        self.thresholds = thresholds;
    }

    /// Record memory allocation
    pub fn record_allocation(&self, size: usize) {
        let new_usage = self.current_usage.load(Ordering::Relaxed) + size as u64;
        self.current_usage.store(new_usage, Ordering::Relaxed);
        self.update_pressure_level();
    }

    /// Record memory deallocation
    pub fn record_deallocation(&self, size: usize) {
        let current = self.current_usage.load(Ordering::Relaxed);
        let new_usage = current.saturating_sub(size as u64);
        self.current_usage.store(new_usage, Ordering::Relaxed);
        self.update_pressure_level();
    }

    /// Get current pressure level
    pub fn get_pressure(&self) -> f64 {
        self.pressure_level.load(Ordering::Relaxed)
    }

    /// Update pressure level based on current usage
    fn update_pressure_level(&self) {
        let current_usage = self.current_usage.load(Ordering::Relaxed);
        let pressure = if self.memory_limit > 0 {
            current_usage as f64 / self.memory_limit as f64
        } else {
            0.0
        };
        self.pressure_level
            .store(pressure.min(1.0), Ordering::Relaxed);
    }

    /// Check if eviction should be triggered based on pressure
    pub fn should_evict(&self) -> bool {
        let pressure = self.pressure_level.load(Ordering::Relaxed);
        pressure > self.thresholds.high_pressure
    }
}

impl crate::cache::tier::warm::memory_monitor_trait::MemoryMonitor for MemoryPressureMonitor {
    fn record_allocation(&self, size: usize) {
        self.record_allocation(size)
    }

    fn record_deallocation(&self, size: usize) {
        self.record_deallocation(size)
    }

    fn get_pressure(&self) -> f64 {
        self.get_pressure()
    }

    fn should_evict(&self) -> bool {
        self.should_evict()
    }

    fn update_memory_usage(
        &self,
        new_usage: u64,
    ) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        self.update_memory_usage(new_usage)
    }

    fn get_current_usage(&self) -> u64 {
        self.get_current_usage()
    }
}
