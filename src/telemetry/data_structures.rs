//! Shared data structures for performance tracking
//!
//! This module contains common data structures used across the performance
//! tracking system for better organization and reusability.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::SystemTime;

use arrayvec::ArrayVec;
use crossbeam_utils::atomic::AtomicCell;

use super::types::PerformanceAlert;

/// Sophisticated performance sample with comprehensive metrics
/// This is the feature-rich version following the sophistication principle
#[derive(Debug, Clone)]
pub struct PerformanceSample {
    /// Timestamp in nanoseconds since epoch
    pub timestamp_ns: u64,
    /// Hit rate scaled by 1000 for precision
    pub hit_rate_x1000: u32,
    /// Average access time in nanoseconds
    pub avg_access_time_ns: u32,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Operations per second scaled by 100 for precision
    pub ops_per_second_x100: u32,
    /// Hot tier utilization scaled by 100
    pub hot_utilization_x100: u16,
    /// Warm tier utilization scaled by 100
    pub warm_utilization_x100: u16,
    /// Cold tier utilization scaled by 100
    pub cold_utilization_x100: u16,
    /// Padding for cache line alignment
    pub _padding: [u8; 6],
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
            _padding: [0; 6],
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

/// Alert rate limiting configuration
#[derive(Debug)]
pub struct AlertRateLimits {
    /// Maximum alerts per minute by type
    pub max_alerts_per_minute: [AtomicU32; 5], // Per AlertType
    /// Current alert counts in window
    pub current_counts: [AtomicU32; 5],
    /// Rate limit window start time
    pub window_start: AtomicU64, // Nanoseconds since epoch
}

impl AlertRateLimits {
    pub fn new() -> Self {
        Self {
            max_alerts_per_minute: [
                AtomicU32::new(10), // HighLatency
                AtomicU32::new(10), // LowHitRate
                AtomicU32::new(5),  // MemoryPressure
                AtomicU32::new(5),  // HighErrorRate
                AtomicU32::new(3),  // SystemOverload
            ],
            current_counts: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],
            window_start: AtomicU64::new(0),
        }
    }
}

/// Alert history for pattern analysis
#[derive(Debug)]
pub struct AlertHistoryBuffer {
    /// Recent alerts with full context
    pub alerts: ArrayVec<PerformanceAlert, 128>,
    /// Alert pattern analysis state
    pub pattern_state: AlertPatternState,
    /// Buffer utilization tracking
    pub utilization: AtomicUsize,
}

impl AlertHistoryBuffer {
    pub fn new() -> Self {
        Self {
            alerts: ArrayVec::new(),
            pattern_state: AlertPatternState::new(),
            utilization: AtomicUsize::new(0),
        }
    }
}

/// Threshold adaptation state for dynamic adjustment
#[derive(Debug)]
pub struct ThresholdAdaptationState {
    /// Adaptation learning rate (rate * 10000)
    pub learning_rate: AtomicU32,
    /// Adaptation direction per threshold
    pub adaptation_directions: [AtomicU32; 5], // Per AlertType, direction * 1000
    /// Last adaptation timestamps
    pub last_adaptations: [AtomicU64; 5], // Nanoseconds since epoch
}

impl ThresholdAdaptationState {
    pub fn new() -> Self {
        Self {
            learning_rate: AtomicU32::new(1000), // 0.1 learning rate * 10000
            adaptation_directions: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],
            last_adaptations: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
        }
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

impl AlertPatternState {
    /// Get the pattern confidence level
    pub fn pattern_confidence(&self) -> f32 {
        self.pattern_confidence.load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Update pattern confidence
    pub fn update_pattern_confidence(&self, confidence: f32) {
        let confidence_scaled = (confidence * 1000.0) as u32;
        self.pattern_confidence
            .store(confidence_scaled, Ordering::Relaxed);
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

/// Alert pattern analysis state
#[derive(Debug)]
pub struct AlertPatternState {
    /// Pattern recognition coefficients
    pub pattern_coefficients: [AtomicU32; 8], // Packed as u32 * 1000
    /// Pattern matching confidence
    pub pattern_confidence: AtomicU32, // Confidence * 1000
    /// Last pattern update timestamp
    pub last_pattern_update: AtomicU64,
}

impl AlertPatternState {
    pub fn new() -> Self {
        Self {
            pattern_coefficients: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],
            pattern_confidence: AtomicU32::new(0),
            last_pattern_update: AtomicU64::new(0),
        }
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
