//! Core performance monitoring system
//!
//! This module provides the main PerformanceMonitor that coordinates
//! metrics collection and alerting.

use super::alert_system::AlertSystem;
use super::metrics_collector::MetricsCollector;
use super::types::{PerformanceAlert, PerformanceSnapshot};

/// Performance monitor for adaptive optimization
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// Performance metrics collector
    metrics_collector: MetricsCollector,
    /// Alert system
    alert_system: AlertSystem,
}

#[allow(dead_code)] // Performance monitoring - comprehensive performance monitoring library with metrics collection and alert system
impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMonitor {
    /// Create new performance monitor
    pub fn new() -> Self {
        Self {
            metrics_collector: MetricsCollector::new(),
            alert_system: AlertSystem::new(),
        }
    }

    /// Collect current performance metrics
    #[inline]
    pub fn collect_metrics(&self) -> PerformanceSnapshot {
        self.metrics_collector.collect_current_snapshot()
    }

    /// Generate comprehensive performance report with minimal allocation
    #[inline]
    pub fn generate_report(&self) -> PerformanceReport {
        let snapshot = self.metrics_collector.collect_current_snapshot();
        let alerts = self.alert_system.get_active_alerts().to_vec();
        let buffer_stats = self.metrics_collector.get_buffer_stats();

        PerformanceReport::new(snapshot, alerts, buffer_stats)
    }

    /// Get current system health status via performance report
    #[inline]
    pub fn health_status(&self) -> (f64, bool) {
        let report = self.generate_report();
        (report.health_score(), report.is_healthy())
    }

    /// Check for performance alerts
    #[inline]
    pub fn check_alerts(&self, snapshot: &PerformanceSnapshot) -> Vec<PerformanceAlert> {
        self.alert_system.evaluate_alerts(snapshot)
    }

    /// Record cache hit for metrics collection
    #[inline(always)]
    pub fn record_hit(&self, access_time_ns: u64) {
        self.metrics_collector.record_hit(access_time_ns);
    }

    /// Record cache miss for metrics collection
    #[inline(always)]
    pub fn record_miss(&self, access_time_ns: u64) {
        self.metrics_collector.record_miss(access_time_ns);
    }

    /// Record memory usage sample
    #[inline]
    pub fn record_memory_usage(&self, memory_bytes: u64) {
        self.metrics_collector.record_memory_usage(memory_bytes);
    }

    /// Force immediate metrics collection
    pub fn force_metrics_collection(&self) -> PerformanceSnapshot {
        self.metrics_collector.force_collection()
    }

    /// Get metrics collector reference
    #[allow(dead_code)] // Performance monitoring - get_metrics_collector used in performance system access
    pub fn get_metrics_collector(&self) -> &MetricsCollector {
        &self.metrics_collector
    }

    /// Get mutable metrics collector reference
    #[allow(dead_code)] // Performance monitoring - get_metrics_collector_mut used in performance system access
    pub fn get_metrics_collector_mut(&mut self) -> &mut MetricsCollector {
        &mut self.metrics_collector
    }

    /// Get alert system reference
    #[allow(dead_code)] // Performance monitoring - get_alert_system used in alert system access
    pub fn get_alert_system(&self) -> &AlertSystem {
        &self.alert_system
    }

    /// Get mutable alert system reference
    #[allow(dead_code)] // Performance monitoring - get_alert_system_mut used in alert system configuration
    pub fn get_alert_system_mut(&mut self) -> &mut AlertSystem {
        &mut self.alert_system
    }

    /// Enable or disable metrics collection
    pub fn set_collection_active(&self, active: bool) {
        self.metrics_collector.set_collection_active(active);
    }

    /// Check if metrics collection is active
    pub fn is_collection_active(&self) -> bool {
        self.metrics_collector.is_collection_active()
    }

    /// Reset all metrics and clear alerts
    pub fn reset_all(&mut self) {
        self.metrics_collector.reset_metrics();
        self.alert_system.clear_active_alerts();
    }
}

/// Performance recommendation
#[allow(dead_code)]
// Performance monitoring - PerformanceRecommendation used in performance analysis and recommendations
#[derive(Debug, Clone)]
pub struct PerformanceRecommendation {
    pub category: RecommendationCategory,
    pub priority: RecommendationPriority,
    pub description: String,
    pub action: String,
}

/// Recommendation category
#[allow(dead_code)]
// Performance monitoring - RecommendationCategory used in performance recommendation classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationCategory {
    HitRate,
    Latency,
    Memory,
    Throughput,
    Configuration,
}

/// Recommendation priority
#[allow(dead_code)]
// Performance monitoring - RecommendationPriority used in performance recommendation prioritization
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Performance report
#[allow(dead_code)] // Performance monitoring - comprehensive performance report structure
#[derive(Debug)]
pub struct PerformanceReport {
    pub current_snapshot: PerformanceSnapshot,
    pub active_alerts: Vec<PerformanceAlert>,
    pub buffer_stats: super::metrics_collector::MetricsBufferStats,
}

#[allow(dead_code)] // Performance monitoring - performance report methods for comprehensive analysis
impl PerformanceReport {
    /// Create new performance report with zero allocation
    #[inline]
    pub fn new(
        snapshot: PerformanceSnapshot,
        alerts: Vec<PerformanceAlert>,
        buffer_stats: super::metrics_collector::MetricsBufferStats,
    ) -> Self {
        Self {
            current_snapshot: snapshot,
            active_alerts: alerts,
            buffer_stats,
        }
    }

    /// Create performance report from existing data with zero copying
    #[inline]
    pub fn from_components(
        snapshot: PerformanceSnapshot,
        alerts: Vec<PerformanceAlert>,
        buffer_stats: super::metrics_collector::MetricsBufferStats,
    ) -> Self {
        Self {
            current_snapshot: snapshot,
            active_alerts: alerts,
            buffer_stats,
        }
    }

    /// Create empty performance report for initialization
    #[inline]
    pub fn empty() -> Self {
        Self {
            current_snapshot: PerformanceSnapshot::default(),
            active_alerts: Vec::new(),
            buffer_stats: super::metrics_collector::MetricsBufferStats::default(),
        }
    }

    /// Calculate overall system health score (0.0 = critical, 1.0 = perfect)
    #[inline]
    pub fn health_score(&self) -> f64 {
        let hit_rate = self.buffer_stats.hit_rate();
        let alert_penalty = match self.active_alerts.len() {
            0 => 1.0,
            1..=2 => 0.9,
            3..=5 => 0.7,
            _ => 0.5,
        };
        hit_rate * alert_penalty
    }

    /// Check if system is operating optimally
    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.health_score() > 0.8 && self.active_alerts.len() <= 2
    }
}
