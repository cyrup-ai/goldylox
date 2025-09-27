//! Shared data structures for performance tracking
//!
//! This module contains common data structures used across the performance
//! tracking system for better organization and reusability.

#![allow(dead_code)] // Telemetry system - comprehensive monitoring infrastructure

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::SystemTime;

use arrayvec::ArrayVec;
use crossbeam_utils::CachePadded;
use crossbeam_utils::atomic::AtomicCell;

use super::types::PerformanceAlert;

/// Enhanced performance sample combining comprehensive metrics with error tracking - CANONICAL VERSION
/// This is the feature-rich version following the sophistication principle, enhanced with error tracking and tier hit detection
#[derive(Debug, Clone, Copy)]
pub struct PerformanceSample {
    /// Timestamp in nanoseconds since epoch (enhanced precision from data_structures version)
    pub timestamp_ns: u64,
    /// Hit rate scaled by 1000 for precision (from data_structures version)
    pub hit_rate_x1000: u32,
    /// Average access time in nanoseconds (from data_structures version)
    pub avg_access_time_ns: u32,
    /// Memory usage in bytes (enhanced from data_structures version)
    pub memory_usage: u64,
    /// Operations per second scaled by 100 for precision (from data_structures version)
    pub ops_per_second_x100: u32,
    /// Hot tier utilization scaled by 100 (from data_structures version)
    pub hot_utilization_x100: u16,
    /// Warm tier utilization scaled by 100 (from data_structures version)
    pub warm_utilization_x100: u16,
    /// Cold tier utilization scaled by 100 (from data_structures version)
    pub cold_utilization_x100: u16,

    // Enhanced fields from types.rs version for error tracking and tier hit detection
    /// Latency measurement in nanoseconds (from types version)
    pub latency_ns: u64,
    /// Throughput as floating point (from types version)
    pub throughput: f64,
    /// Error count tracking (from types version)
    pub error_count: u32,
    /// Operation-specific latency in nanoseconds (from types version)
    pub operation_latency_ns: u64,
    /// Tier hit detection flag (from types version)
    pub tier_hit: bool,

    /// Padding for cache line alignment (reduced due to additional fields)
    pub _padding: [u8; 3],
}

impl Default for PerformanceSample {
    fn default() -> Self {
        Self {
            timestamp_ns: 0,
            hit_rate_x1000: 0,
            avg_access_time_ns: 0,
            memory_usage: 0,
            ops_per_second_x100: 0,
            hot_utilization_x100: 0,
            warm_utilization_x100: 0,
            cold_utilization_x100: 0,
            // Enhanced fields from types.rs version
            latency_ns: 0,
            throughput: 0.0,
            error_count: 0,
            operation_latency_ns: 0,
            tier_hit: false,
            _padding: [0; 3],
        }
    }
}

/// Operations per second calculation state
#[derive(Debug)]
pub struct OpsPerSecondState {
    /// Last calculation timestamp
    last_calculation: AtomicU64, // Nanoseconds since epoch
    /// Operations count at last calculation
    last_ops_count: AtomicU64,
    /// Current ops per second value (packed * 100)
    current_ops_per_sec: AtomicU32,
    /// Moving average window size
    window_size: u32,
}

/// Tier hit rate calculation state
#[derive(Debug)]
pub struct TierHitRateState {
    /// Hit rates per tier (rate * 1000 for precision)
    tier_rates: [AtomicU32; 3], // Hot, Warm, Cold
    /// Last update timestamps per tier
    last_updates: [AtomicU64; 3], // Nanoseconds since epoch
    /// Update frequencies per tier
    update_frequencies: [AtomicU32; 3], // Updates per second * 100
}

/// Atomic collection state for metrics collector
#[derive(Debug)]
pub struct CollectionState {
    /// Collection active flag
    pub is_collecting: AtomicCell<bool>,
    /// Collection thread coordination
    pub collection_generation: AtomicU64,
    /// Error count for collection failures
    pub error_count: AtomicU32,
}

impl CollectionState {
    pub fn new() -> Self {
        Self {
            is_collecting: AtomicCell::new(false),
            collection_generation: AtomicU64::new(0),
            error_count: AtomicU32::new(0),
        }
    }
}

/// Trend history buffer with efficient storage
#[derive(Debug)]
pub struct TrendHistoryBuffer {
    /// Trend samples with timestamp
    pub samples: ArrayVec<TrendSample, 256>,
    /// Buffer write position
    pub write_pos: AtomicUsize,
    /// Sample count for analysis
    pub sample_count: AtomicUsize,
}

impl TrendHistoryBuffer {
    pub fn new() -> Self {
        Self {
            samples: ArrayVec::new(),
            write_pos: AtomicUsize::new(0),
            sample_count: AtomicUsize::new(0),
        }
    }
}

/// Alert rate limiting with per-type granular controls (CANONICAL VERSION)
/// Enhanced with cache-padded optimization for maximum performance
#[derive(Debug)]
pub struct AlertRateLimits {
    /// Maximum alerts per minute by type (cache-padded for performance)
    pub max_alerts_per_minute: CachePadded<[AtomicU32; 5]>, // Per AlertType
    /// Current alert counts in window (cache-padded for performance)
    pub current_counts: CachePadded<[AtomicU32; 5]>,
    /// Rate limit window start time (cache-padded for performance)
    pub window_start: CachePadded<AtomicU64>, // Nanoseconds since epoch
}

impl Default for AlertRateLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertRateLimits {
    /// Create with default per-type rate limits
    pub fn new() -> Self {
        Self {
            max_alerts_per_minute: CachePadded::new([
                AtomicU32::new(10), // HighLatency
                AtomicU32::new(10), // LowHitRate
                AtomicU32::new(5),  // MemoryPressure
                AtomicU32::new(5),  // HighErrorRate
                AtomicU32::new(3),  // SystemOverload
            ]),
            current_counts: CachePadded::new([
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ]),
            window_start: CachePadded::new(AtomicU64::new(0)),
        }
    }

    /// Create with custom rate limits for all types
    pub fn with_limits(limits: [u32; 5]) -> Self {
        Self {
            max_alerts_per_minute: CachePadded::new([
                AtomicU32::new(limits[0]), // HighLatency
                AtomicU32::new(limits[1]), // LowHitRate
                AtomicU32::new(limits[2]), // MemoryPressure
                AtomicU32::new(limits[3]), // HighErrorRate
                AtomicU32::new(limits[4]), // SystemOverload
            ]),
            current_counts: CachePadded::new([
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ]),
            window_start: CachePadded::new(AtomicU64::new(0)),
        }
    }

    /// Create with single rate limit applied to all types (backward compatibility)
    pub fn with_single_limit(max_per_minute: u32) -> Self {
        Self::with_limits([max_per_minute; 5])
    }
}

/// Alert history for pattern analysis
#[derive(Debug)]
pub struct AlertHistoryBuffer {
    /// Recent alerts with full context
    pub alerts: ArrayVec<PerformanceAlert, 128>,
    /// Alert pattern analysis state
    pub pattern_state: crate::cache::manager::performance::alert_system::AlertPatternState,
    /// Buffer utilization tracking
    pub utilization: AtomicUsize,
}

impl AlertHistoryBuffer {
    pub fn new() -> Self {
        Self {
            alerts: ArrayVec::new(),
            pattern_state: crate::cache::manager::performance::alert_system::AlertPatternState::new(
            ),
            utilization: AtomicUsize::new(0),
        }
    }

    /// Push alert to buffer with capacity management (enhanced from telemetry/types.rs)
    pub fn push(&mut self, alert: PerformanceAlert) {
        if self.alerts.is_full() {
            // ArrayVec is full, remove oldest (first) alert
            self.alerts.remove(0);
        }
        // Add new alert to the end
        let _ = self.alerts.try_push(alert);

        // Update utilization atomically
        self.utilization.store(self.alerts.len(), Ordering::Relaxed);
    }

    /// Get current buffer size
    pub fn len(&self) -> usize {
        self.alerts.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.alerts.is_empty()
    }

    /// Get buffer capacity
    pub fn capacity(&self) -> usize {
        self.alerts.capacity()
    }
}

/// Enhanced threshold adaptation state combining atomic thread safety with rich ML features
#[derive(Debug)]
pub struct ThresholdAdaptationState {
    /// Adaptation learning rate (rate * 10000 for precision)
    #[allow(dead_code)]
    // ML system - used in adaptive threshold tuning and machine learning feedback loops
    pub learning_rate: AtomicU32,
    /// Adaptation directions per alert type (direction * 1000)
    #[allow(dead_code)]
    // ML system - used in adaptive threshold tuning and machine learning feedback loops
    pub adaptation_directions: [AtomicU32; 5], // Per AlertType
    /// Last adaptation timestamps (nanoseconds since epoch)
    #[allow(dead_code)]
    // ML system - used in adaptive threshold tuning and machine learning feedback loops
    pub last_adaptations: [AtomicU64; 5],
    /// Baseline values per alert type (scaled by 1000 for atomic storage)
    #[allow(dead_code)]
    // ML system - used in adaptive threshold tuning and machine learning feedback loops
    pub baseline_values: [AtomicU32; 5], // Baseline values * 1000
    /// Adaptation factor for sensitivity tuning (factor * 1000)
    #[allow(dead_code)]
    // ML system - used in adaptive threshold tuning and machine learning feedback loops
    pub adaptation_factor: AtomicU32,
}

impl ThresholdAdaptationState {
    pub fn new() -> Self {
        Self {
            learning_rate: AtomicU32::new(1000), // 0.1 learning rate * 10000
            adaptation_directions: [const { AtomicU32::new(0) }; 5],
            last_adaptations: [const { AtomicU64::new(0) }; 5],
            baseline_values: [const { AtomicU32::new(0) }; 5], // No baseline initially
            adaptation_factor: AtomicU32::new(1000),           // 1.0 * 1000
        }
    }

    /// Set baseline value for specific alert type (thread-safe)
    #[inline]
    pub fn set_baseline(&self, alert_type_idx: usize, baseline: f64) {
        if alert_type_idx < 5 {
            self.baseline_values[alert_type_idx]
                .store((baseline * 1000.0) as u32, Ordering::Relaxed);
        }
    }

    /// Get baseline value for specific alert type
    #[inline]
    pub fn get_baseline(&self, alert_type_idx: usize) -> f64 {
        if alert_type_idx < 5 {
            self.baseline_values[alert_type_idx].load(Ordering::Relaxed) as f64 / 1000.0
        } else {
            0.0
        }
    }

    /// Update adaptation factor
    #[inline]
    pub fn set_adaptation_factor(&self, factor: f64) {
        self.adaptation_factor
            .store((factor * 1000.0) as u32, Ordering::Relaxed);
    }

    /// Get current adaptation factor
    #[inline]
    pub fn get_adaptation_factor(&self) -> f64 {
        self.adaptation_factor.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get current learning rate
    #[inline]
    pub fn get_learning_rate(&self) -> f64 {
        self.learning_rate.load(Ordering::Relaxed) as f64 / 10000.0
    }

    /// Set learning rate (thread-safe)
    #[inline]
    pub fn set_learning_rate(&self, rate: f64) {
        self.learning_rate
            .store((rate * 10000.0) as u32, Ordering::Relaxed);
    }
}

/// Performance history retention policy
#[derive(Debug)]
pub struct RetentionPolicy {
    /// Maximum age for samples (nanoseconds)
    pub max_sample_age: u64,
    /// Cleanup frequency (nanoseconds between cleanup)
    pub cleanup_frequency: u64,
    /// Last cleanup timestamp
    pub last_cleanup: AtomicU64,
}

impl RetentionPolicy {
    pub fn new() -> Self {
        Self {
            max_sample_age: 24 * 60 * 60 * 1_000_000_000, // 24 hours in nanoseconds
            cleanup_frequency: 60 * 60 * 1_000_000_000,   // 1 hour in nanoseconds
            last_cleanup: AtomicU64::new(0),
        }
    }
}

/// Trend sample for analysis
#[derive(Debug, Clone, Copy)]
pub struct TrendSample {
    /// Sample timestamp
    timestamp: u64, // Nanoseconds since epoch
    /// Performance metric value
    pub value: f32,
    /// Hit rate scaled by 1000 for precision
    pub hit_rate_x1000: u32,
    /// Average access time in nanoseconds
    pub avg_access_time_ns: u64,
    /// Memory usage in bytes
    pub memory_usage: usize,
    /// Operations per second scaled by 100 for precision
    pub ops_per_second_x100: u32,
    /// Trend direction (-1.0 to 1.0)
    direction: f32,
    /// Trend strength (0.0 to 1.0)
    strength: f32,
}

impl TrendHistoryBuffer {
    /// Get the current sample count
    pub fn sample_count(&self) -> usize {
        self.sample_count.load(Ordering::Relaxed)
    }

    /// Get the current write position
    pub fn write_pos(&self) -> usize {
        self.write_pos.load(Ordering::Relaxed)
    }

    /// Reset the buffer
    pub fn reset(&self) {
        self.write_pos.store(0, Ordering::Relaxed);
        self.sample_count.store(0, Ordering::Relaxed);
    }
}

impl TrendSample {
    /// Create a new trend sample
    pub fn new(
        timestamp: u64,
        value: f32,
        hit_rate_x1000: u32,
        avg_access_time_ns: u64,
        memory_usage: usize,
        ops_per_second_x100: u32,
    ) -> Self {
        Self {
            timestamp,
            value,
            hit_rate_x1000,
            avg_access_time_ns,
            memory_usage,
            ops_per_second_x100,
            direction: 0.0,
            strength: 0.0,
        }
    }

    /// Get the sample timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get the performance metric value
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Get the trend direction
    pub fn direction(&self) -> f32 {
        self.direction
    }

    /// Get the trend strength
    pub fn strength(&self) -> f32 {
        self.strength
    }
}

impl OpsPerSecondState {
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            last_calculation: AtomicU64::new(now),
            last_ops_count: AtomicU64::new(0),
            current_ops_per_sec: AtomicU32::new(0),
            window_size: 10, // 10-second moving window
        }
    }

    pub fn get_current_ops_per_second(&self) -> f32 {
        self.current_ops_per_sec.load(Ordering::Relaxed) as f32 / 100.0
    }

    pub fn reset(&self) {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        self.last_calculation.store(now, Ordering::Relaxed);
        self.last_ops_count.store(0, Ordering::Relaxed);
        self.current_ops_per_sec.store(0, Ordering::Relaxed);
    }
}

impl TierHitRateState {
    pub fn new() -> Self {
        Self {
            tier_rates: [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],
            last_updates: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],
            update_frequencies: [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],
        }
    }

    pub fn record_hit(&self, tier_idx: usize, _access_time_ns: u64) {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        self.last_updates[tier_idx].store(now, Ordering::Relaxed);
        self.update_frequencies[tier_idx].fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_tier_rate(&self, tier_idx: usize) -> f32 {
        self.tier_rates[tier_idx].load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub fn reset(&self) {
        for i in 0..3 {
            self.tier_rates[i].store(0, Ordering::Relaxed);
            self.last_updates[i].store(0, Ordering::Relaxed);
            self.update_frequencies[i].store(0, Ordering::Relaxed);
        }
    }
}
