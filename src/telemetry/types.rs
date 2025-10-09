#![allow(dead_code)]
// Telemetry System - Complete telemetry types library with measurement results, performance alerts, monitoring configuration, trend analysis, and comprehensive type definitions

//! Telemetry types module - consolidated from performance_tracking, measurement, and cache/performance_tracking

use std::time::{Duration, Instant};

/// Measurement result types
#[derive(Debug, Clone, Copy, Default)]
pub struct MeasurementResult {
    pub width: f32,
    pub height: f32,
}

/// Measurement statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct MeasurementStats {
    pub total_measurements: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_time_ns: u64,
}

// AlertSeverity moved to canonical location: crate::cache::types::core_types::AlertSeverity
pub use crate::cache::types::core_types::AlertSeverity;

/// Alert type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertType {
    MemoryPressure,
    PerformanceDegradation,
    ErrorRateSpike,
    ResourceExhaustion,
    ThroughputDrop,
    LowHitRate,
    HighLatency,
    HighMemoryUsage,
}

/// Performance alert structure
#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: Instant,
    pub value: f64,
    pub threshold: f64,
    pub metric_value: f64,
}

// PerformanceSample moved to canonical location: crate::telemetry::data_structures::PerformanceSample
// Use the enhanced canonical implementation with comprehensive tier metrics plus error tracking and tier hit detection
pub use crate::telemetry::data_structures::PerformanceSample;

/// Monitor configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub sample_interval: Duration,
    pub history_size: usize,
    pub adaptive_thresholds_active: bool,
    pub sample_interval_ms: u64,
    /// Maximum alerts per minute for rate limiting (default: 60)
    pub alert_rate_limit_per_minute: u32,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            sample_interval: Duration::from_millis(100),
            history_size: 1000,
            adaptive_thresholds_active: true,
            sample_interval_ms: 100,
            alert_rate_limit_per_minute: 60,
        }
    }
}

// AlertHistoryBuffer moved to canonical location: crate::telemetry::data_structures::AlertHistoryBuffer

// AlertRateLimits moved to canonical location: crate::telemetry::data_structures::AlertRateLimits
// Use the canonical implementation with enhanced per-type rate limiting and CachePadded optimization
pub use crate::telemetry::data_structures::AlertRateLimits;

// ThresholdAdaptationState moved to canonical location: crate::telemetry::data_structures::ThresholdAdaptationState
// Use the canonical implementation with enhanced thread-safe atomic fields and rich ML features

// TrendSample moved to canonical location: crate::telemetry::data_structures::TrendSample
// Use the canonical implementation with comprehensive performance metrics including hit rate, latency, memory, throughput, trend direction, and strength

/// Performance trends analysis
#[derive(Debug, Clone)]
pub struct PerformanceTrends {
    pub latency_trend: f64,
    pub throughput_trend: f64,
    pub memory_trend: f64,
    pub error_rate_trend: f64,
    pub hit_rate_trend: f64,
    pub prediction_confidence: f64,
}

impl Default for PerformanceTrends {
    fn default() -> Self {
        Self {
            latency_trend: 0.0,
            throughput_trend: 0.0,
            memory_trend: 0.0,
            error_rate_trend: 0.0,
            hit_rate_trend: 0.0,
            prediction_confidence: 0.0,
        }
    }
}
