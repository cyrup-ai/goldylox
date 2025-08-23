//! Alert system for performance monitoring
//!
//! This module handles performance alert evaluation, notification management,
//! and alert history tracking for cache performance monitoring.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::types::{
    AlertConfig, AlertEvent, AlertEventType, AlertSeverity, AlertThresholds, AlertType,
    PerformanceAlert, PerformanceSnapshot,
};

/// Alert system for performance monitoring
#[derive(Debug)]
pub struct AlertSystem {
    /// Alert thresholds
    thresholds: AlertThresholds,
    /// Active alerts
    active_alerts: Vec<PerformanceAlert>,
    /// Alert history
    alert_history: VecDeque<AlertEvent>,
    /// Alert configuration
    alert_config: AlertConfig,
    /// Notification status
    notification_enabled: AtomicBool,
    /// Next alert ID
    next_alert_id: std::sync::atomic::AtomicU64,
}

impl AlertSystem {
    /// Create new alert system
    pub fn new() -> Self {
        Self {
            thresholds: AlertThresholds::default(),
            active_alerts: Vec::new(),
            alert_history: VecDeque::new(),
            alert_config: AlertConfig::default(),
            notification_enabled: AtomicBool::new(true),
            next_alert_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Evaluate alerts based on performance snapshot
    #[inline]
    pub fn evaluate_alerts(&self, snapshot: &PerformanceSnapshot) -> Vec<PerformanceAlert> {
        let mut alerts = Vec::new();

        // Check hit rate degradation
        self.check_hit_rate_alerts(snapshot, &mut alerts);

        // Check latency spikes
        self.check_latency_alerts(snapshot, &mut alerts);

        // Check memory pressure
        self.check_memory_alerts(snapshot, &mut alerts);

        // Check throughput degradation
        self.check_throughput_alerts(snapshot, &mut alerts);

        alerts
    }

    /// Trigger performance alert
    #[inline]
    pub fn trigger_alert(&mut self, alert: PerformanceAlert) {
        // Check if we already have this type of alert active
        if self.is_alert_type_active(alert.alert_type) {
            return;
        }

        // Add to active alerts
        self.active_alerts.push(alert.clone());

        // Add to history
        let event = AlertEvent {
            timestamp: Instant::now(),
            alert: alert.clone(),
            event_type: AlertEventType::Triggered,
        };
        self.alert_history.push_back(event);

        // Send notification if enabled
        if self.notification_enabled.load(Ordering::Relaxed) {
            self.send_notification(&alert);
        }

        // Trim history if needed
        self.trim_alert_history();
    }

    /// Get all active alerts
    pub fn get_active_alerts(&self) -> &[PerformanceAlert] {
        &self.active_alerts
    }

    /// Clear all active alerts
    pub fn clear_active_alerts(&mut self) {
        for alert in self.active_alerts.drain(..) {
            let event = AlertEvent {
                timestamp: Instant::now(),
                alert,
                event_type: AlertEventType::Resolved,
            };
            self.alert_history.push_back(event);
        }
    }

    /// Get alert system statistics
    pub fn get_alert_stats(&self) -> AlertSystemStats {
        let mut severity_counts = [0; 4];
        let mut type_counts = [0; 6];

        for alert in &self.active_alerts {
            let severity_idx = match alert.severity {
                AlertSeverity::Info => 0,
                AlertSeverity::Warning => 1,
                AlertSeverity::Critical => 2,
                AlertSeverity::Emergency => 3,
            };
            severity_counts[severity_idx] += 1;

            let type_idx = match alert.alert_type {
                AlertType::HitRateDegradation => 0,
                AlertType::LatencySpike => 1,
                AlertType::MemoryPressure => 2,
                AlertType::ThroughputDrop => 3,
                AlertType::EfficiencyDegradation => 4,
                AlertType::SystemOverload => 5,
            };
            type_counts[type_idx] += 1;
        }

        AlertSystemStats {
            active_alerts_count: self.active_alerts.len(),
            total_history_count: self.alert_history.len(),
            info_alerts: severity_counts[0],
            warning_alerts: severity_counts[1],
            critical_alerts: severity_counts[2],
            emergency_alerts: severity_counts[3],
            hit_rate_alerts: type_counts[0],
            latency_alerts: type_counts[1],
            memory_alerts: type_counts[2],
            throughput_alerts: type_counts[3],
            efficiency_alerts: type_counts[4],
            system_overload_alerts: type_counts[5],
        }
    }

    // Private helper methods

    fn check_hit_rate_alerts(
        &self,
        snapshot: &PerformanceSnapshot,
        alerts: &mut Vec<PerformanceAlert>,
    ) {
        let critical_threshold =
            self.thresholds.critical_hit_rate.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let warning_threshold =
            self.thresholds.warning_hit_rate.load(Ordering::Relaxed) as f64 / 1_000_000.0;

        if snapshot.hit_rate < critical_threshold {
            alerts.push(self.create_alert(
                AlertType::HitRateDegradation,
                AlertSeverity::Critical,
                format!(
                    "Hit rate {:.2}% below critical threshold {:.2}%",
                    snapshot.hit_rate * 100.0,
                    critical_threshold * 100.0
                ),
                snapshot.hit_rate,
                critical_threshold,
            ));
        } else if snapshot.hit_rate < warning_threshold {
            alerts.push(self.create_alert(
                AlertType::HitRateDegradation,
                AlertSeverity::Warning,
                format!(
                    "Hit rate {:.2}% below warning threshold {:.2}%",
                    snapshot.hit_rate * 100.0,
                    warning_threshold * 100.0
                ),
                snapshot.hit_rate,
                warning_threshold,
            ));
        }
    }

    fn check_latency_alerts(
        &self,
        snapshot: &PerformanceSnapshot,
        alerts: &mut Vec<PerformanceAlert>,
    ) {
        let max_latency = self.thresholds.max_latency_ns.load(Ordering::Relaxed);

        if snapshot.avg_latency_ns > max_latency {
            let severity = if snapshot.avg_latency_ns > max_latency * 2 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            };

            alerts.push(self.create_alert(
                AlertType::LatencySpike,
                severity,
                format!(
                    "Average latency {:.2}ms exceeds threshold {:.2}ms",
                    snapshot.avg_latency_ns as f64 / 1_000_000.0,
                    max_latency as f64 / 1_000_000.0
                ),
                snapshot.avg_latency_ns as f64,
                max_latency as f64,
            ));
        }
    }

    fn check_memory_alerts(
        &self,
        snapshot: &PerformanceSnapshot,
        alerts: &mut Vec<PerformanceAlert>,
    ) {
        let memory_threshold = self
            .thresholds
            .memory_warning_threshold
            .load(Ordering::Relaxed) as f64
            / 1_000_000.0;

        if snapshot.memory_utilization > memory_threshold {
            let severity = if snapshot.memory_utilization > 0.95 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            };

            alerts.push(self.create_alert(
                AlertType::MemoryPressure,
                severity,
                format!(
                    "Memory utilization {:.1}% exceeds threshold {:.1}%",
                    snapshot.memory_utilization * 100.0,
                    memory_threshold * 100.0
                ),
                snapshot.memory_utilization,
                memory_threshold,
            ));
        }
    }

    fn check_throughput_alerts(
        &self,
        snapshot: &PerformanceSnapshot,
        alerts: &mut Vec<PerformanceAlert>,
    ) {
        let min_throughput = 1000; // ops/sec

        if snapshot.throughput_ops_per_sec < min_throughput {
            alerts.push(self.create_alert(
                AlertType::ThroughputDrop,
                AlertSeverity::Warning,
                format!(
                    "Throughput {} ops/sec below minimum threshold {} ops/sec",
                    snapshot.throughput_ops_per_sec, min_throughput
                ),
                snapshot.throughput_ops_per_sec as f64,
                min_throughput as f64,
            ));
        }
    }

    fn create_alert(
        &self,
        alert_type: AlertType,
        severity: AlertSeverity,
        message: String,
        current_value: f64,
        threshold_value: f64,
    ) -> PerformanceAlert {
        PerformanceAlert {
            alert_id: self.next_alert_id.fetch_add(1, Ordering::Relaxed),
            alert_type,
            severity,
            message,
            triggered_at: Instant::now(),
            current_value,
            threshold_value,
        }
    }

    fn is_alert_type_active(&self, alert_type: AlertType) -> bool {
        self.active_alerts
            .iter()
            .any(|alert| alert.alert_type == alert_type)
    }

    fn send_notification(&self, alert: &PerformanceAlert) {
        // In a real implementation, this would send notifications via email, Slack, etc.
        println!("ALERT: {:?} - {}", alert.severity, alert.message);
    }

    fn trim_alert_history(&mut self) {
        const MAX_HISTORY_SIZE: usize = 1000;

        while self.alert_history.len() > MAX_HISTORY_SIZE {
            self.alert_history.pop_front();
        }
    }
}

/// Statistics about the alert system
#[derive(Debug, Clone)]
pub struct AlertSystemStats {
    pub active_alerts_count: usize,
    pub total_history_count: usize,
    pub info_alerts: usize,
    pub warning_alerts: usize,
    pub critical_alerts: usize,
    pub emergency_alerts: usize,
    pub hit_rate_alerts: usize,
    pub latency_alerts: usize,
    pub memory_alerts: usize,
    pub throughput_alerts: usize,
    pub efficiency_alerts: usize,
    pub system_overload_alerts: usize,
}
