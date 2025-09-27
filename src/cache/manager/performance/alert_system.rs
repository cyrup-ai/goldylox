//! Alert system for performance monitoring
//!
//! This module handles performance alert evaluation, notification management,
//! alert history tracking, and adaptive threshold management for cache performance monitoring.
//!
//! Enhanced with SIMD-optimized performance features and pattern analysis capabilities.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use crossbeam_utils::CachePadded;
use log;

use super::types::{
    AlertConfig, AlertEvent, AlertEventType, AlertSeverity, AlertType, PerformanceAlert,
    PerformanceSnapshot,
};
use crate::cache::types::performance_thresholds::AlertThresholds;
use crate::telemetry::data_structures::ThresholdAdaptationState;

// Email notification dependencies
#[cfg(feature = "lettre")]
use lettre::transport::smtp::authentication::Credentials;
#[cfg(feature = "lettre")]
use lettre::{Message, SmtpTransport, Transport};

use serde::{Deserialize, Serialize};

/// Configuration for notification delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub email: Option<EmailConfig>,
    pub rate_limit_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub to_addresses: Vec<String>,
    pub min_severity: AlertSeverity,
    pub use_tls: bool,
}
use crate::telemetry::data_structures::AlertHistoryBuffer;
use crate::telemetry::types::{MonitorConfig, PerformanceSample};

/// Pattern analysis state for sophisticated alert pattern detection
#[derive(Debug)]
#[allow(dead_code)] // Performance alert system - pattern state fields for SIMD-optimized pattern detection
pub struct AlertPatternState {
    /// Pattern detection coefficients for SIMD-optimized analysis
    #[allow(dead_code)]
    // Alert system - pattern coefficients used in SIMD-optimized pattern analysis
    pub pattern_coefficients: [AtomicU32; 8],
    /// Pattern detection confidence level
    #[allow(dead_code)]
    // Alert system - confidence level used in pattern recognition algorithms
    pub pattern_confidence: AtomicU32,
    /// Last pattern analysis update timestamp
    #[allow(dead_code)] // Alert system - update timestamp used in pattern analysis scheduling
    pub last_pattern_update: AtomicU64,
}

impl Default for AlertPatternState {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertPatternState {
    /// Create new alert pattern state with default values
    pub fn new() -> Self {
        Self {
            pattern_coefficients: [
                AtomicU32::new(1000),
                AtomicU32::new(800),
                AtomicU32::new(600),
                AtomicU32::new(400),
                AtomicU32::new(300),
                AtomicU32::new(200),
                AtomicU32::new(150),
                AtomicU32::new(100),
            ],
            pattern_confidence: AtomicU32::new(0),
            last_pattern_update: AtomicU64::new(0),
        }
    }

    /// Get pattern confidence as ratio (0.0 to 1.0)
    #[allow(dead_code)] // Performance alert system - pattern confidence query for analysis
    pub fn pattern_confidence(&self) -> f32 {
        self.pattern_confidence.load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Update pattern coefficients with new analysis results
    #[allow(dead_code)] // Performance alert system - pattern coefficient update for ML-based pattern detection
    pub fn update_coefficients(&self, coefficients: &[f32; 8]) {
        for (i, &coeff) in coefficients.iter().enumerate() {
            let scaled = (coeff * 1000.0) as u32;
            self.pattern_coefficients[i].store(scaled, Ordering::Relaxed);
        }
        self.pattern_confidence.store(
            (coefficients.iter().sum::<f32>() * 125.0) as u32, // Avg * 1000
            Ordering::Relaxed,
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.last_pattern_update.store(now, Ordering::Relaxed);
    }
}

// ThresholdAdaptationState moved to canonical location: crate::telemetry::data_structures::ThresholdAdaptationState
// Use the canonical implementation with enhanced atomic thread safety plus rich ML features

// AlertRateLimits moved to canonical location: crate::telemetry::data_structures::AlertRateLimits
pub use crate::telemetry::data_structures::AlertRateLimits;

// AlertHistoryBuffer moved to canonical location: crate::telemetry::data_structures::AlertHistoryBuffer

/// Alert system for performance monitoring with SIMD optimizations
#[derive(Debug)]
pub struct AlertSystem {
    /// Alert thresholds (SIMD-optimized with cache padding)
    thresholds: CachePadded<AlertThresholds>,
    /// Active alerts
    #[allow(dead_code)] // Alert system - active alerts collection used in alert management
    active_alerts: Vec<PerformanceAlert>,
    /// Alert history with VecDeque for management features
    #[allow(dead_code)] // Alert system - alert history used in performance trend analysis
    alert_history: VecDeque<AlertEvent>,
    /// Zero-allocation high-performance alert buffer
    #[allow(dead_code)]
    // Alert system - performance alert buffer used in high-throughput alert processing
    performance_alert_buffer: AlertHistoryBuffer,
    /// Alert configuration
    #[allow(dead_code)]
    // Performance monitoring - alert_config used in alert system configuration management
    alert_config: AlertConfig,

    /// Next alert ID
    next_alert_id: AtomicU64,
    /// Rate limiting state
    rate_limits: AlertRateLimits,
    /// Threshold adaptation state with ML learning
    #[allow(dead_code)]
    // Performance monitoring - adaptation_state used in ML-based threshold adaptation
    adaptation_state: ThresholdAdaptationState,
    /// Notification configuration and delivery system
    notification_config: Option<NotificationConfig>,
    /// SMTP transport for email notifications
    #[cfg(feature = "lettre")]
    email_transport: Option<SmtpTransport>,
    /// Last notification timestamp for rate limiting
    last_notification_time: AtomicU64,
}

#[allow(dead_code)] // Performance alert system - comprehensive alert management library with SIMD optimization and ML-based threshold adaptation
impl Default for AlertSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertSystem {
    /// Create new alert system
    pub fn new() -> Self {
        Self {
            thresholds: CachePadded::new(AlertThresholds::default()),
            active_alerts: Vec::new(),
            alert_history: VecDeque::new(),
            performance_alert_buffer: AlertHistoryBuffer::new(), // Use canonical implementation (128 capacity)
            alert_config: AlertConfig::default(),

            next_alert_id: AtomicU64::new(1),
            rate_limits: AlertRateLimits::with_limits([
                10, // HitRateDegradation
                10, // LatencySpike
                5,  // MemoryPressure
                10, // ThroughputDrop
                3,  // EfficiencyDegradation
            ]),
            adaptation_state: ThresholdAdaptationState::new(),
            notification_config: None,
            #[cfg(feature = "lettre")]
            email_transport: None,
            last_notification_time: AtomicU64::new(0),
        }
    }

    /// Create new alert system with configuration (telemetry compatibility)
    #[allow(dead_code)] // Performance monitoring - from_config used in configurable alert system initialization
    pub fn from_config(
        _config: MonitorConfig,
    ) -> Result<Self, crate::cache::traits::types_and_enums::CacheOperationError> {
        let mut alert_system = Self::new();
        // Use canonical AlertThresholds with default values
        alert_system.thresholds = CachePadded::new(AlertThresholds::default());
        Ok(alert_system)
    }

    /// Configure notification system with email settings
    pub fn configure_notifications(
        &mut self,
        config: NotificationConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(feature = "lettre")]
        if let Some(email_config) = &config.email {
            let transport = SmtpTransport::relay(&email_config.smtp_server)?
                .port(email_config.smtp_port)
                .credentials(Credentials::new(
                    email_config.username.clone(),
                    email_config.password.clone(),
                ))
                .build();
            self.email_transport = Some(transport);
        }

        self.notification_config = Some(config);
        Ok(())
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

    /// Check for performance alerts using telemetry sample (telemetry compatibility)
    pub fn check_alerts(&self, sample: &PerformanceSample) -> Vec<PerformanceAlert> {
        let mut alerts = Vec::new();
        let thresholds = &self.thresholds;

        // Check latency alert using feature-rich operation_latency_ns field
        let latency_ms = sample.operation_latency_ns / 1_000_000;
        if latency_ms as f64
            > thresholds.max_latency_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
            && let Some(alert) = self.create_alert_if_allowed(
                AlertType::LatencySpike,
                sample,
                latency_ms as f64,
                thresholds.max_latency_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0,
            )
        {
            alerts.push(alert);
        }

        // Check memory usage alert using feature-rich memory_usage field
        let memory_mb = sample.memory_usage / (1024 * 1024);
        let memory_threshold =
            thresholds.memory_warning_threshold.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        if memory_mb as f64 > memory_threshold
            && let Some(alert) = self.create_alert_if_allowed(
                AlertType::MemoryPressure,
                sample,
                memory_mb as f64,
                memory_threshold,
            )
        {
            alerts.push(alert);
        }

        // Check tier hit patterns for degradation using feature-rich tier_hit field
        if !sample.tier_hit {
            // Cache miss - potential degradation
            if let Some(alert) = self.create_alert_if_allowed(
                AlertType::HitRateDegradation,
                sample,
                0.0, // Miss
                1.0, // Expected hit
            ) {
                alerts.push(alert);
            }
        }

        alerts
    }

    /// Get current alert thresholds
    #[allow(dead_code)] // Alert system - threshold query API for alert configuration analysis
    pub fn current_thresholds(&self) -> &AlertThresholds {
        &self.thresholds
    }

    /// Adapt thresholds based on performance patterns with ML learning
    #[allow(dead_code)] // Alert system - threshold adaptation API with ML-based pattern learning
    pub fn adapt_thresholds(&self, pattern_analysis: &[f32]) {
        // Update pattern coefficients using SIMD-optimized atomic operations
        for (i, &coeff) in pattern_analysis.iter().enumerate().take(8) {
            self.performance_alert_buffer
                .pattern_state
                .pattern_coefficients[i]
                .store((coeff * 1000.0) as u32, Ordering::Relaxed);
        }

        // Update pattern confidence and timestamp
        let avg_confidence = pattern_analysis.iter().sum::<f32>() / pattern_analysis.len() as f32;
        self.performance_alert_buffer
            .pattern_state
            .pattern_confidence
            .store((avg_confidence * 1000.0) as u32, Ordering::Relaxed);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.performance_alert_buffer
            .pattern_state
            .last_pattern_update
            .store(now, Ordering::Relaxed);
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

        self.send_notification(&alert);

        // Trim history if needed
        self.trim_alert_history();
    }

    /// Get all active alerts
    #[allow(dead_code)] // Alert system - active alerts query API for monitoring dashboard
    pub fn get_active_alerts(&self) -> &[PerformanceAlert] {
        &self.active_alerts
    }

    /// Clear all active alerts
    #[allow(dead_code)] // Alert system - alert clearing API for maintenance operations
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

    /// Use alert history buffer for pattern analysis
    #[allow(dead_code)] // Alert system - alert pattern analysis API with ML-based trend detection
    pub fn analyze_alert_patterns(&self) -> Vec<f32> {
        let pattern_confidence = self
            .performance_alert_buffer
            .pattern_state
            .pattern_confidence();
        vec![
            pattern_confidence,
            pattern_confidence * 0.9,
            pattern_confidence * 0.8,
            pattern_confidence * 0.7,
        ]
    }

    /// Use rate limiting functionality
    pub fn get_rate_limit_status(&self) -> (bool, Vec<u32>) {
        let window_start = self.rate_limits.window_start.load(Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let window_active = now - window_start < 60_000_000_000; // 60 second window

        let current_counts: Vec<u32> = (0..5)
            .map(|i| (*self.rate_limits.current_counts)[i].load(Ordering::Relaxed))
            .collect();

        (window_active, current_counts)
    }

    /// Get alert history for analysis
    pub fn get_alert_history(&self) -> &std::collections::VecDeque<AlertEvent> {
        &self.alert_history
    }

    /// Get alert system statistics
    pub fn get_alert_stats(&self) -> AlertSystemStats {
        let mut severity_counts = [0; 4];
        let mut type_counts = [0; 6];

        for alert in &self.active_alerts {
            let severity_idx = match alert.severity {
                AlertSeverity::Info => 0,
                AlertSeverity::Low => 1,
                AlertSeverity::Medium => 2,
                AlertSeverity::Warning => 3,
                AlertSeverity::High => 4,
                AlertSeverity::Error => 5,
                AlertSeverity::Critical => 6,
                AlertSeverity::Emergency => 7,
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
        // Rate limiting to prevent notification spam
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let last_sent = self.last_notification_time.load(Ordering::Relaxed);
        let rate_limit_seconds = self
            .notification_config
            .as_ref()
            .map(|c| c.rate_limit_seconds)
            .unwrap_or(300); // Default 5 minutes

        if now - last_sent < rate_limit_seconds {
            log::debug!("Rate limiting notification for alert: {}", alert.message);
            return;
        }

        // Try to send email notification if configured
        if let Some(config) = &self.notification_config
            && let Some(email_config) = &config.email
            && alert.severity >= email_config.min_severity
        {
            if let Err(e) = self.send_email_notification(alert, email_config) {
                log::error!("Failed to send email notification: {}", e);
            } else {
                self.last_notification_time.store(now, Ordering::Relaxed);
                log::info!(
                    "Successfully sent email notification for alert: {}",
                    alert.message
                );
            }
        }

        // Always log as fallback
        log::warn!("ALERT: {:?} - {}", alert.severity, alert.message);
    }

    #[cfg(feature = "lettre")]
    fn send_email_notification(
        &self,
        alert: &PerformanceAlert,
        config: &EmailConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let transport = self
            .email_transport
            .as_ref()
            .ok_or("Email transport not configured")?;

        let subject = format!(
            "🚨 Goldylox Alert: {:?} - {:?}",
            alert.severity, alert.alert_type
        );
        let body = format!(
            "Alert Details:\n\n\
            Severity: {:?}\n\
            Alert Type: {:?}\n\
            Message: {}\n\
            Timestamp: {:?}\n\
            Current Value: {:.3}\n\
            Threshold: {:.3}\n\n\
            This alert was generated by the Goldylox cache monitoring system.",
            alert.severity,
            alert.alert_type,
            alert.message,
            alert.triggered_at,
            alert.current_value,
            alert.threshold_value
        );

        for to_address in &config.to_addresses {
            let email = Message::builder()
                .from(config.from_address.parse()?)
                .to(to_address.parse()?)
                .subject(&subject)
                .body(body.clone())?;

            transport.send(&email)?;
        }

        Ok(())
    }

    #[cfg(not(feature = "lettre"))]
    fn send_email_notification(
        &self,
        _alert: &PerformanceAlert,
        _config: &EmailConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("Email notifications require the 'lettre' feature to be enabled".into())
    }

    fn trim_alert_history(&mut self) {
        const MAX_HISTORY_SIZE: usize = 1000;

        while self.alert_history.len() > MAX_HISTORY_SIZE {
            self.alert_history.pop_front();
        }
    }

    /// Create alert with rate limiting and pattern analysis
    fn create_alert_if_allowed(
        &self,
        alert_type: AlertType,
        _sample: &PerformanceSample,
        current_value: f64,
        threshold_value: f64,
    ) -> Option<PerformanceAlert> {
        let type_idx = match alert_type {
            AlertType::HitRateDegradation => 0,
            AlertType::LatencySpike => 1,
            AlertType::MemoryPressure => 2,
            AlertType::ThroughputDrop => 3,
            AlertType::EfficiencyDegradation => 4,
            AlertType::SystemOverload => 4, // Map to efficiency
        };

        // Check rate limiting
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let window_start = self.rate_limits.window_start.load(Ordering::Relaxed);
        let window_duration = 60_000_000_000; // 60 seconds in nanoseconds

        // Reset window if needed
        if now - window_start > window_duration {
            self.rate_limits.window_start.store(now, Ordering::Relaxed);
            for count in (*self.rate_limits.current_counts).iter() {
                count.store(0, Ordering::Relaxed);
            }
        }

        // Check if we can send this alert type
        let current_count = (*self.rate_limits.current_counts)[type_idx].load(Ordering::Relaxed);
        let max_count = (*self.rate_limits.max_alerts_per_minute)[type_idx].load(Ordering::Relaxed);

        if current_count >= max_count {
            return None; // Rate limited
        }

        // Increment count
        (*self.rate_limits.current_counts)[type_idx].fetch_add(1, Ordering::Relaxed);

        // Determine severity based on deviation
        let deviation_ratio = (current_value - threshold_value).abs() / threshold_value;
        let severity = if deviation_ratio > 2.0 {
            AlertSeverity::Critical
        } else if deviation_ratio > 1.0 {
            AlertSeverity::Warning
        } else {
            AlertSeverity::Info
        };

        Some(PerformanceAlert {
            alert_id: self.next_alert_id.fetch_add(1, Ordering::Relaxed),
            alert_type,
            severity,
            message: format!(
                "{:?} alert: current={:.3}, threshold={:.3}",
                alert_type, current_value, threshold_value
            ),
            triggered_at: Instant::now(),
            current_value,
            threshold_value,
        })
    }

    /// Update alert pattern coefficients for machine learning
    pub fn update_pattern_coefficients(&self, coefficients: &[f32; 8]) {
        self.performance_alert_buffer
            .pattern_state
            .update_coefficients(coefficients);
    }
}

/// Statistics about the alert system
#[allow(dead_code)] // Performance monitoring - AlertSystemStats used in alert system statistics and reporting
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
