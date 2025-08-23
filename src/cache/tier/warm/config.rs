//! Configuration types for warm tier cache behavior
//!
//! This module contains all configuration structures for controlling warm tier
//! cache behavior including memory pressure thresholds, eviction policies,
//! access tracking, background tasks, and performance tuning parameters.

use super::eviction::types::EvictionPolicyType;

/// Configuration for warm tier cache behavior
#[derive(Debug, Clone)]
pub struct WarmTierConfig {
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    /// Maximum number of entries
    pub max_entries: usize,
    /// Default TTL for entries
    pub default_ttl_sec: u64,
    /// Memory pressure thresholds
    pub pressure_thresholds: PressureConfig,
    /// Eviction policy configuration
    pub eviction_config: EvictionConfig,
    /// Access tracking configuration
    pub tracking_config: TrackingConfig,
    /// Background task configuration
    pub background_config: BackgroundConfig,
    /// Performance tuning parameters
    pub performance_config: PerformanceConfig,
}

/// Memory pressure configuration thresholds
#[derive(Debug, Clone)]
pub struct PressureConfig {
    /// Low pressure threshold (0.0-1.0)
    pub low_threshold: f64,
    /// Medium pressure threshold
    pub medium_threshold: f64,
    /// High pressure threshold
    pub high_threshold: f64,
    /// Critical pressure threshold
    pub critical_threshold: f64,
    /// Alert cooldown period in milliseconds
    pub alert_cooldown_ms: u64,
    /// Memory leak detection sensitivity
    pub leak_detection_sensitivity: f64,
}

/// Eviction policy configuration
#[derive(Debug, Clone)]
pub struct EvictionConfig {
    /// Primary eviction policy
    pub primary_policy: EvictionPolicyType,
    /// Enable adaptive policy switching
    pub adaptive_switching: bool,
    /// Policy evaluation interval in seconds
    pub evaluation_interval_sec: u64,
    /// Performance history window size
    pub performance_window_size: usize,
    /// Minimum confidence for policy switches
    pub switch_confidence_threshold: f64,
    /// Eviction batch size for bulk operations
    pub eviction_batch_size: usize,
}

/// Access pattern tracking configuration
#[derive(Debug, Clone)]
pub struct TrackingConfig {
    /// Access history window size
    pub history_window_size: usize,
    /// Pattern analysis interval in seconds
    pub pattern_analysis_interval_sec: u64,
    /// Frequency estimation parameters
    pub frequency_estimation: FrequencyConfig,
    /// Pattern classification sensitivity
    pub pattern_sensitivity: f64,
    /// Enable predictive prefetching
    pub enable_prefetching: bool,
}

/// Frequency estimation configuration
#[derive(Debug, Clone)]
pub struct FrequencyConfig {
    /// Exponential moving average decay factor
    pub decay_factor: f64,
    /// Minimum frequency resolution (Hz)
    pub min_frequency_hz: f64,
    /// Maximum frequency to track (Hz)
    pub max_frequency_hz: f64,
    /// Sample window size for variance calculation
    pub sample_window_size: usize,
}

/// Background task processing configuration
#[derive(Debug, Clone)]
pub struct BackgroundConfig {
    /// Enable background processing
    pub enable_background_tasks: bool,
    /// Task processing interval in milliseconds
    pub task_interval_ms: u64,
    /// Maximum tasks per processing cycle
    pub max_tasks_per_cycle: usize,
    /// Background thread pool size
    pub thread_pool_size: usize,
    /// Task queue capacity
    pub task_queue_capacity: usize,
}

/// Performance tuning configuration
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Enable SIMD optimization where available
    pub enable_simd: bool,
    /// Cache line alignment for atomic structures
    pub cache_line_alignment: usize,
    /// Skiplist layer probability
    pub skiplist_probability: f64,
    /// Memory prefetch hints
    pub enable_prefetch_hints: bool,
    /// Batch operation sizes
    pub batch_sizes: BatchSizeConfig,
    /// Concurrency limits
    pub concurrency_limits: ConcurrencyConfig,
}

/// Batch operation size configuration
#[derive(Debug, Clone)]
pub struct BatchSizeConfig {
    /// Cleanup batch size
    pub cleanup_batch: usize,
    /// Eviction batch size
    pub eviction_batch: usize,
    /// Statistics update batch size
    pub stats_batch: usize,
    /// Pattern analysis batch size
    pub analysis_batch: usize,
}

/// Concurrency control configuration
#[derive(Debug, Clone)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent readers
    pub max_readers: usize,
    /// Maximum concurrent writers
    pub max_writers: usize,
    /// Reader/writer balance ratio
    pub rw_balance_ratio: f64,
    /// Contention backoff parameters
    pub backoff_config: BackoffConfig,
}

/// Backoff strategy configuration for contention handling
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial backoff delay in nanoseconds
    pub initial_delay_ns: u64,
    /// Maximum backoff delay in nanoseconds
    pub max_delay_ns: u64,
    /// Backoff multiplier
    pub multiplier: f64,
    /// Jitter factor for randomization
    pub jitter_factor: f64,
}

/// Default configuration implementations
impl Default for WarmTierConfig {
    #[inline]
    fn default() -> Self {
        Self {
            max_memory_bytes: 256 * 1024 * 1024, // 256MB
            max_entries: 10_000,
            default_ttl_sec: 3600, // 1 hour
            pressure_thresholds: PressureConfig::default(),
            eviction_config: EvictionConfig::default(),
            tracking_config: TrackingConfig::default(),
            background_config: BackgroundConfig::default(),
            performance_config: PerformanceConfig::default(),
        }
    }
}

impl Default for PressureConfig {
    #[inline]
    fn default() -> Self {
        Self {
            low_threshold: 0.5,
            medium_threshold: 0.7,
            high_threshold: 0.85,
            critical_threshold: 0.95,
            alert_cooldown_ms: 30_000,
            leak_detection_sensitivity: 0.8,
        }
    }
}

impl Default for EvictionConfig {
    #[inline]
    fn default() -> Self {
        Self {
            primary_policy: EvictionPolicyType::Adaptive,
            adaptive_switching: true,
            evaluation_interval_sec: 60,
            performance_window_size: 100,
            switch_confidence_threshold: 0.8,
            eviction_batch_size: 32,
        }
    }
}

impl Default for TrackingConfig {
    #[inline]
    fn default() -> Self {
        Self {
            history_window_size: 1000,
            pattern_analysis_interval_sec: 30,
            frequency_estimation: FrequencyConfig::default(),
            pattern_sensitivity: 0.7,
            enable_prefetching: true,
        }
    }
}

impl Default for FrequencyConfig {
    #[inline]
    fn default() -> Self {
        Self {
            decay_factor: 0.9,
            min_frequency_hz: 0.001,
            max_frequency_hz: 1000.0,
            sample_window_size: 32,
        }
    }
}

impl Default for BackgroundConfig {
    #[inline]
    fn default() -> Self {
        Self {
            enable_background_tasks: true,
            task_interval_ms: 100,
            max_tasks_per_cycle: 10,
            thread_pool_size: 2,
            task_queue_capacity: 1000,
        }
    }
}

impl Default for PerformanceConfig {
    #[inline]
    fn default() -> Self {
        Self {
            enable_simd: true,
            cache_line_alignment: 64,
            skiplist_probability: 0.5,
            enable_prefetch_hints: true,
            batch_sizes: BatchSizeConfig::default(),
            concurrency_limits: ConcurrencyConfig::default(),
        }
    }
}

impl Default for BatchSizeConfig {
    #[inline]
    fn default() -> Self {
        Self {
            cleanup_batch: 100,
            eviction_batch: 50,
            stats_batch: 25,
            analysis_batch: 200,
        }
    }
}

impl Default for ConcurrencyConfig {
    #[inline]
    fn default() -> Self {
        Self {
            max_readers: 1000,
            max_writers: 10,
            rw_balance_ratio: 0.8,
            backoff_config: BackoffConfig::default(),
        }
    }
}

impl Default for BackoffConfig {
    #[inline]
    fn default() -> Self {
        Self {
            initial_delay_ns: 1000,
            max_delay_ns: 1_000_000,
            multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}
