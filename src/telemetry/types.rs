//! Telemetry types module - consolidated from performance_tracking, measurement, and cache/performance_tracking

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64};
use std::time::{Duration, Instant};

use crossbeam_utils::CachePadded;

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

/// Alert thresholds configuration
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub memory_pressure_threshold: f64,
    pub performance_degradation_threshold: f64,
    pub error_rate_threshold: f64,
    pub throughput_threshold: f64,
    pub max_latency_ms: f64,
    pub max_memory_mb: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            memory_pressure_threshold: 0.8,
            performance_degradation_threshold: 0.5,
            error_rate_threshold: 0.05,
            throughput_threshold: 0.7,
            max_latency_ms: 100.0,
            max_memory_mb: 512.0,
        }
    }
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

/// Performance sample for monitoring
#[derive(Debug, Clone, Copy)]
pub struct PerformanceSample {
    pub timestamp: u64,
    pub latency_ns: u64,
    pub throughput: f64,
    pub memory_usage: usize,
    pub error_count: u32,
    pub operation_latency_ns: u64,
    pub tier_hit: bool,
}

/// Monitor configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub sample_interval: Duration,
    pub history_size: usize,
    pub alert_thresholds: AlertThresholds,
    pub enable_adaptive_thresholds: bool,
    pub sample_interval_ms: u64,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            sample_interval: Duration::from_millis(100),
            history_size: 1000,
            alert_thresholds: AlertThresholds::default(),
            enable_adaptive_thresholds: true,
            sample_interval_ms: 100,
        }
    }
}

/// Alert history buffer
#[derive(Debug)]
pub struct AlertHistoryBuffer {
    pub alerts: VecDeque<PerformanceAlert>,
    pub max_size: usize,
}

impl AlertHistoryBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            alerts: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, alert: PerformanceAlert) {
        if self.alerts.len() >= self.max_size {
            self.alerts.pop_front();
        }
        self.alerts.push_back(alert);
    }
}

/// Alert rate limits
#[derive(Debug)]
pub struct AlertRateLimits {
    pub max_alerts_per_minute: u32,
    pub current_count: CachePadded<AtomicU32>,
    pub last_reset: CachePadded<AtomicU64>,
}

impl AlertRateLimits {
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            max_alerts_per_minute: max_per_minute,
            current_count: CachePadded::new(AtomicU32::new(0)),
            last_reset: CachePadded::new(AtomicU64::new(0)),
        }
    }
}

/// Threshold adaptation state
#[derive(Debug)]
pub struct ThresholdAdaptationState {
    pub baseline_values: Vec<f64>,
    pub adaptation_factor: f64,
    pub last_adaptation: Instant,
    pub learning_rate: f64,
    pub adaptive_enabled: bool,
}

impl ThresholdAdaptationState {
    pub fn new() -> Self {
        Self {
            baseline_values: Vec::new(),
            adaptation_factor: 1.0,
            last_adaptation: Instant::now(),
            learning_rate: 0.1,
            adaptive_enabled: true,
        }
    }
}

/// Trend sample for analysis
#[derive(Debug, Clone, Copy)]
pub struct TrendSample {
    pub timestamp: u64,
    pub value: f64,
    pub derivative: f64,
}

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
