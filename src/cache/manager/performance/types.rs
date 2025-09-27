//! Type definitions for performance monitoring
//!
//! This module contains all the core types, enums, and configuration
//! structures used throughout the performance monitoring system.

#![allow(dead_code)] // Performance monitoring - comprehensive metrics and alerting system

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, Instant};

// AlertThresholds import removed - not used in this module

/// Performance snapshot containing key metrics
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    /// Timestamp when snapshot was taken
    pub timestamp: Instant,
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Average access latency in nanoseconds
    pub avg_latency_ns: u64,
    /// Memory utilization ratio (0.0 to 1.0)
    pub memory_utilization: f64,
    /// Throughput in operations per second
    pub throughput_ops_per_sec: u32,
    /// Overall efficiency score (0.0 to 1.0)
    pub efficiency_score: f64,
}

impl Default for PerformanceSnapshot {
    fn default() -> Self {
        Self {
            timestamp: Instant::now(),
            hit_rate: 0.0,
            avg_latency_ns: 0,
            memory_utilization: 0.0,
            throughput_ops_per_sec: 0,
            efficiency_score: 0.0,
        }
    }
}

/// Trend direction for performance metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    /// Performance is improving
    Improving,
    /// Performance is stable
    Stable,
    /// Performance is degrading
    Degrading,
}

// AlertSeverity moved to canonical location: crate::cache::types::core_types::AlertSeverity
pub use crate::cache::types::core_types::AlertSeverity;

/// Types of performance alerts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertType {
    HitRateDegradation,
    LatencySpike,
    MemoryPressure,
    ThroughputDrop,
    EfficiencyDegradation,
    SystemOverload,
}

/// Performance alert
#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    /// Unique alert ID
    pub alert_id: u64,
    /// Type of alert
    pub alert_type: AlertType,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert message
    pub message: String,
    /// When alert was triggered
    pub triggered_at: Instant,
    /// Current metric value
    pub current_value: f64,
    /// Threshold value that was exceeded
    pub threshold_value: f64,
}

/// Alert event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertEventType {
    Triggered,
    Resolved,
}

/// Alert event for history tracking
#[derive(Debug, Clone)]
pub struct AlertEvent {
    /// Event timestamp
    pub timestamp: Instant,
    /// Associated alert
    pub alert: PerformanceAlert,
    /// Type of event
    pub event_type: AlertEventType,
}

/// Alert configuration
#[derive(Debug, Clone)]
pub struct AlertConfig {
    /// Maximum number of active alerts
    pub max_active_alerts: usize,
    /// Alert evaluation interval
    pub evaluation_interval: Duration,
    /// Email notifications active
    pub email_notifications_active: bool,
    /// Console notifications active
    pub console_notifications_active: bool,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            max_active_alerts: 50,
            evaluation_interval: Duration::from_secs(30),
            email_notifications_active: false,
            console_notifications_active: true,
        }
    }
}

/// History compression configuration
#[derive(Debug, Clone)]
pub struct HistoryCompressionConfig {
    /// Compression of old data active
    pub compression_active: bool,
    /// Age threshold for compression
    pub compression_threshold: Duration,
    /// Compression ratio target
    pub compression_ratio: f64,
}

impl Default for HistoryCompressionConfig {
    fn default() -> Self {
        Self {
            compression_active: true,
            compression_threshold: Duration::from_secs(3600), // 1 hour
            compression_ratio: 0.1,
        }
    }
}

/// History storage statistics
#[derive(Debug)]
pub struct HistoryStorageStats {
    /// Total snapshots stored
    pub total_snapshots: AtomicU64,
    /// Compressed snapshots count
    pub compressed_snapshots: AtomicU64,
    /// Estimated memory usage in bytes
    pub estimated_memory_bytes: AtomicU64,
}

impl Default for HistoryStorageStats {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryStorageStats {
    pub fn new() -> Self {
        Self {
            total_snapshots: AtomicU64::new(0),
            compressed_snapshots: AtomicU64::new(0),
            estimated_memory_bytes: AtomicU64::new(0),
        }
    }
}

/// Metrics collection configuration
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Collection interval
    pub collection_interval: Duration,
    /// Buffer size for metrics
    pub buffer_size: usize,
    /// Enable automatic collection
    pub auto_collection: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collection_interval: Duration::from_secs(1),
            buffer_size: 1000,
            auto_collection: true,
        }
    }
}

/// Atomic metrics for thread-safe collection
#[derive(Debug)]
pub struct AtomicMetrics {
    /// Total cache hits
    pub total_hits: AtomicU64,
    /// Total cache misses
    pub total_misses: AtomicU64,
    /// Total access time in nanoseconds
    pub total_access_time_ns: AtomicU64,
    /// Current memory usage in bytes
    pub current_memory_bytes: AtomicU64,
    /// Collection active flag
    pub collection_active: AtomicBool,
}

impl Default for AtomicMetrics {
    fn default() -> Self {
        Self {
            total_hits: AtomicU64::new(0),
            total_misses: AtomicU64::new(0),
            total_access_time_ns: AtomicU64::new(0),
            current_memory_bytes: AtomicU64::new(0),
            collection_active: AtomicBool::new(true),
        }
    }
}

/// Metrics buffer for storing recent samples
#[derive(Debug)]
pub struct MetricsBuffer {
    /// Buffer of performance snapshots
    pub snapshots: VecDeque<PerformanceSnapshot>,
    /// Maximum buffer size
    pub max_size: usize,
    /// Total samples processed
    pub total_processed: AtomicU64,
}

impl MetricsBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(max_size),
            max_size,
            total_processed: AtomicU64::new(0),
        }
    }
}

/// Trend analysis configuration
#[derive(Debug, Clone)]
pub struct TrendConfig {
    /// Sample window size for trend analysis
    pub sample_window_size: usize,
    /// Minimum samples required for trend analysis
    pub min_samples_for_trend: usize,
    /// Trend sensitivity (0.0 to 1.0)
    pub trend_sensitivity: f64,
}

impl Default for TrendConfig {
    fn default() -> Self {
        Self {
            sample_window_size: 50,
            min_samples_for_trend: 5,
            trend_sensitivity: 0.1,
        }
    }
}
