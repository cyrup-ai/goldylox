#![allow(dead_code)]
// Warm tier config - Complete configuration library with memory pressure thresholds, eviction policies, access tracking, and performance tuning

//! Configuration types for warm tier cache behavior
//!
//! This module contains all configuration structures for controlling warm tier
//! cache behavior including memory pressure thresholds, eviction policies,
//! access tracking, background tasks, and performance tuning parameters.

use super::eviction::types::EvictionPolicyType;
use serde::{Deserialize, Serialize};

/// Skip map configuration for warm tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipMapConfig {
    pub max_level: u8,
    pub skip_probability_x1000: u16,
    pub node_pool_size: u32,
}

/// Configuration for warm tier cache behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(align(64))]
pub struct WarmTierConfig {
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    /// Maximum number of entries
    pub max_entries: usize,
    /// Default TTL for entries
    pub default_ttl_sec: u64,
    /// Promotion threshold for tier transitions
    pub promotion_threshold: u16,
    /// Demotion age threshold in nanoseconds
    pub demotion_age_threshold_ns: u64,
    /// Skip map data structure configuration
    pub skip_map: SkipMapConfig,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Unified eviction configuration for all cache tiers
/// Combines best features from hot and warm tier implementations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionConfig {
    // Core Policy Configuration
    /// Primary eviction policy
    pub primary_policy: EvictionPolicyType,
    /// Fallback policy for edge cases
    pub fallback_policy: EvictionPolicyType,

    // Adaptive Intelligence (from Warm tier)
    /// Enable adaptive policy switching based on performance
    pub adaptive_switching: bool,
    /// Machine learning policy optimization
    /// Policy evaluation interval in seconds
    pub evaluation_interval_sec: u64,
    /// Performance analysis window size
    pub performance_window_size: usize,
    /// Minimum confidence threshold for policy switches
    pub switch_confidence_threshold: f64,

    // Performance Optimization (from Hot tier)
    /// Historical data buffer size for analysis
    pub history_buffer_size: usize,
    /// Performance threshold for triggering adaptations
    pub performance_threshold: f64,
    /// Adaptation check interval (converted to seconds)
    pub adaptation_interval_sec: u64,

    // Batch Operations (from Warm tier)
    /// Eviction batch size for bulk operations
    pub eviction_batch_size: usize,
    /// Maximum items to evaluate per cycle
    pub max_evaluation_items: usize,

    // Tier-Specific Overrides
    /// Hot tier specific settings
    pub hot_tier_overrides: Option<HotTierEvictionOverrides>,
    /// Warm tier specific settings  
    pub warm_tier_overrides: Option<WarmTierEvictionOverrides>,
}

/// Hot tier specific eviction overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotTierEvictionOverrides {
    /// Force specific policy for hot tier
    pub force_policy: Option<EvictionPolicyType>,
    /// Hot tier specific batch size
    pub batch_size_override: Option<usize>,
}

/// Warm tier specific eviction overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmTierEvictionOverrides {
    /// Extended evaluation window for warm tier
    pub extended_window_size: Option<usize>,
    /// Custom confidence threshold for warm tier
    pub confidence_override: Option<f64>,
}

/// Access pattern tracking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingConfig {
    /// Access history window size
    pub history_window_size: usize,
    /// Pattern analysis interval in seconds
    pub pattern_analysis_interval_sec: u64,
    /// Frequency estimation parameters
    pub frequency_estimation: FrequencyConfig,
    /// Pattern classification sensitivity
    pub pattern_sensitivity: f64,
    /// Predictive prefetching
    pub prefetching_active: bool,
}

/// Frequency estimation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundConfig {
    /// Background processing
    pub background_tasks_active: bool,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// SIMD optimization where available
    pub simd_active: bool,
    /// Cache line alignment for atomic structures
    pub cache_line_alignment: usize,
    /// Skiplist layer probability
    pub skiplist_probability: f64,
    /// Memory prefetch hints
    pub prefetch_hints_active: bool,
    /// Batch operation sizes
    pub batch_sizes: BatchSizeConfig,
    /// Concurrency limits
    pub concurrency_limits: ConcurrencyConfig,
}

/// Batch operation size configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            promotion_threshold: 100,
            demotion_age_threshold_ns: 3_600_000_000_000, // 1 hour
            skip_map: SkipMapConfig::default(),
            pressure_thresholds: PressureConfig::default(),
            eviction_config: EvictionConfig::default(),
            tracking_config: TrackingConfig::default(),
            background_config: BackgroundConfig::default(),
            performance_config: PerformanceConfig::default(),
        }
    }
}

impl Default for SkipMapConfig {
    #[inline]
    fn default() -> Self {
        Self {
            max_level: 16,
            skip_probability_x1000: 500, // 0.5 probability
            node_pool_size: 1024,
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

impl EvictionConfig {
    /// Create default eviction configuration (const-evaluable)
    #[inline]
    pub const fn default_const() -> Self {
        Self {
            primary_policy: EvictionPolicyType::Adaptive,
            fallback_policy: EvictionPolicyType::Lru,
            adaptive_switching: true,

            evaluation_interval_sec: 60,
            performance_window_size: 1000,
            switch_confidence_threshold: 0.8,
            history_buffer_size: 1000,
            performance_threshold: 0.8,
            adaptation_interval_sec: 60,
            eviction_batch_size: 32,
            max_evaluation_items: 512,
            hot_tier_overrides: None,
            warm_tier_overrides: None,
        }
    }
}

impl Default for EvictionConfig {
    #[inline]
    fn default() -> Self {
        Self::default_const()
    }
}

impl EvictionConfig {
    /// Get effective configuration for hot tier
    pub fn for_hot_tier(&self) -> HotTierEvictionConfig {
        HotTierEvictionConfig {
            policy: self
                .hot_tier_overrides
                .as_ref()
                .and_then(|o| o.force_policy)
                .unwrap_or(self.primary_policy),

            history_size: self.history_buffer_size,
            adaptation_interval_sec: self.adaptation_interval_sec,
            performance_threshold: self.performance_threshold,
            batch_size: self
                .hot_tier_overrides
                .as_ref()
                .and_then(|o| o.batch_size_override)
                .unwrap_or(self.eviction_batch_size),
        }
    }

    /// Get effective configuration for warm tier
    pub fn for_warm_tier(&self) -> WarmTierEvictionConfig {
        WarmTierEvictionConfig {
            primary_policy: self.primary_policy,
            adaptive_switching: self.adaptive_switching,

            evaluation_interval_sec: self.evaluation_interval_sec,
            performance_window_size: self
                .warm_tier_overrides
                .as_ref()
                .and_then(|o| o.extended_window_size)
                .unwrap_or(self.performance_window_size),
            switch_confidence_threshold: self
                .warm_tier_overrides
                .as_ref()
                .and_then(|o| o.confidence_override)
                .unwrap_or(self.switch_confidence_threshold),
            eviction_batch_size: self.eviction_batch_size,
        }
    }
}

/// Hot tier specific configuration adapter
#[derive(Debug, Clone)]
pub struct HotTierEvictionConfig {
    pub policy: EvictionPolicyType,

    pub history_size: usize,
    pub adaptation_interval_sec: u64,
    pub performance_threshold: f64,
    pub batch_size: usize,
}

/// Warm tier specific configuration adapter
#[derive(Debug, Clone)]
pub struct WarmTierEvictionConfig {
    pub primary_policy: EvictionPolicyType,
    pub adaptive_switching: bool,

    pub evaluation_interval_sec: u64,
    pub performance_window_size: usize,
    pub switch_confidence_threshold: f64,
    pub eviction_batch_size: usize,
}

impl Default for TrackingConfig {
    #[inline]
    fn default() -> Self {
        Self {
            history_window_size: 1000,
            pattern_analysis_interval_sec: 30,
            frequency_estimation: FrequencyConfig::default(),
            pattern_sensitivity: 0.7,
            prefetching_active: true,
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
            background_tasks_active: true,
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
            simd_active: true,
            cache_line_alignment: 64,
            skiplist_probability: 0.5,
            prefetch_hints_active: true,
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
