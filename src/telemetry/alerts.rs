//! Alert system with adaptive threshold adjustment
//!
//! This module implements a sophisticated alerting system that can
//! dynamically adjust thresholds based on performance patterns.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize};

use arrayvec::ArrayVec;
use crossbeam_utils::CachePadded;

use super::data_structures::{
    AlertHistoryBuffer, AlertPatternState, AlertRateLimits, ThresholdAdaptationState,
};
// Use feature-rich types from local types.rs module
use super::types::{
    AlertSeverity, AlertThresholds, AlertType, MonitorConfig, PerformanceAlert, PerformanceSample,
};
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Alert system with adaptive threshold adjustment
#[derive(Debug)]
pub struct AlertSystem {
    /// Current alert thresholds (adaptive)
    current_thresholds: CachePadded<AlertThresholds>,
    /// Alert generation rate limits
    rate_limits: AlertRateLimits,
    /// Alert history for pattern analysis
    alert_history: AlertHistoryBuffer,
    /// Threshold adaptation state
    adaptation_state: ThresholdAdaptationState,
}

impl AlertSystem {
    /// Create new alert system with configuration
    pub fn new(_config: MonitorConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            current_thresholds: CachePadded::new(AlertThresholds::default()),
            rate_limits: AlertRateLimits {
                max_alerts_per_minute: [
                    AtomicU32::new(10), // HitRateLow
                    AtomicU32::new(10), // AccessTimeBig
                    AtomicU32::new(5),  // MemoryHigh
                    AtomicU32::new(10), // OpsLow
                    AtomicU32::new(3),  // ErrorRateHigh
                ],
                current_counts: [
                    AtomicU32::new(0),
                    AtomicU32::new(0),
                    AtomicU32::new(0),
                    AtomicU32::new(0),
                    AtomicU32::new(0),
                ],
                window_start: AtomicU64::new(0),
            },
            alert_history: AlertHistoryBuffer {
                alerts: ArrayVec::new(),
                pattern_state: AlertPatternState {
                    pattern_coefficients: [
                        AtomicU32::new(1000),
                        AtomicU32::new(800),
                        AtomicU32::new(600),
                        AtomicU32::new(400),
                        AtomicU32::new(200),
                        AtomicU32::new(0),
                        AtomicU32::new(0),
                        AtomicU32::new(0),
                    ],
                    pattern_confidence: AtomicU32::new(500), // 50%
                    last_pattern_update: AtomicU64::new(0),
                },
                utilization: AtomicUsize::new(0),
            },
            adaptation_state: ThresholdAdaptationState {
                learning_rate: AtomicU32::new(100), // 0.01 * 10000
                adaptation_directions: [
                    AtomicU32::new(1000), // Neutral direction
                    AtomicU32::new(1000),
                    AtomicU32::new(1000),
                    AtomicU32::new(1000),
                    AtomicU32::new(1000),
                ],
                last_adaptations: [
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                ],
            },
        })
    }

    /// Check for performance alerts against current thresholds using feature-rich types
    pub fn check_alerts(&self, sample: &PerformanceSample) -> Vec<PerformanceAlert> {
        let mut alerts = Vec::new();
        let thresholds = &self.current_thresholds;

        // Check latency alert using feature-rich operation_latency_ns field
        let latency_ms = sample.operation_latency_ns / 1_000_000;
        if latency_ms as f64 > thresholds.max_latency_ms {
            if let Some(alert) = self.create_alert_if_allowed(
                AlertType::HighLatency,
                sample,
                latency_ms as f64,
                thresholds.max_latency_ms as f64,
            ) {
                alerts.push(alert);
            }
        }

        // Check memory usage alert using feature-rich memory_usage field
        let memory_mb = sample.memory_usage / (1024 * 1024);
        if memory_mb as f64 > thresholds.max_memory_mb {
            if let Some(alert) = self.create_alert_if_allowed(
                AlertType::HighMemoryUsage,
                sample,
                memory_mb as f64,
                thresholds.max_memory_mb as f64,
            ) {
                alerts.push(alert);
            }
        }

        // Check tier hit patterns for degradation using feature-rich tier_hit field
        if !sample.tier_hit {
            // Cache miss - potential degradation
            if let Some(alert) = self.create_alert_if_allowed(
                AlertType::LowHitRate,
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
    pub fn current_thresholds(&self) -> &AlertThresholds {
        &self.current_thresholds
    }

    /// Adapt thresholds based on performance patterns
    pub fn adapt_thresholds(&self, _pattern_analysis: &[f32]) {
        // Use the EXISTING sophisticated threshold adaptation from strategy module
        use crate::cache::manager::strategy::thresholds::StrategyThresholds;
        
        // The EXISTING StrategyThresholds::adapt() expects:
        // - system_load: 0.0 to 1.0 representing load
        // - stability_ratio: 0.0 to 1.0 representing stability
        
        // Pattern analysis values represent alert frequencies/patterns
        // Higher values = more alerts = higher load, lower stability
        let avg_pattern = if !_pattern_analysis.is_empty() {
            _pattern_analysis.iter().sum::<f32>() / _pattern_analysis.len() as f32
        } else {
            0.5
        };
        
        let system_load = avg_pattern.clamp(0.0, 1.0);
        let stability_ratio = 1.0 - avg_pattern.clamp(0.0, 1.0); // Inverse relationship
        
        let strategy_thresholds = StrategyThresholds::new();
        strategy_thresholds.adapt(system_load, stability_ratio);
    }

    /// Create alert if rate limiting allows using feature-rich types
    fn create_alert_if_allowed(
        &self,
        alert_type: AlertType,
        _sample: &PerformanceSample,
        metric_value: f64,
        threshold_value: f64,
    ) -> Option<PerformanceAlert> {
        use crate::cache::types::timestamp_nanos;
        use std::sync::atomic::Ordering;
        
        let type_idx = alert_type as usize;
        let now = timestamp_nanos(std::time::Instant::now());
        
        // Check window expiry (60 seconds)
        let window_start = self.rate_limits.window_start.load(Ordering::Relaxed);
        if now - window_start > 60_000_000_000 {
            // Reset window
            self.rate_limits.window_start.store(now, Ordering::Relaxed);
            for count in &self.rate_limits.current_counts {
                count.store(0, Ordering::Relaxed);
            }
        }
        
        // Check rate limit
        let current = self.rate_limits.current_counts[type_idx].load(Ordering::Relaxed);
        let max = self.rate_limits.max_alerts_per_minute[type_idx].load(Ordering::Relaxed);
        
        if current < max {
            self.rate_limits.current_counts[type_idx].fetch_add(1, Ordering::Relaxed);
            Some(PerformanceAlert {
                alert_type,
                severity: AlertSeverity::Warning,
                message: format!(
                    "Performance alert: metric {:.2} exceeded threshold {:.2}",
                    metric_value, threshold_value
                ),
                timestamp: std::time::Instant::now(),
                metric_value,
                value: metric_value,
                threshold: threshold_value,
            })
        } else {
            None // Rate limited
        }
    }
}
