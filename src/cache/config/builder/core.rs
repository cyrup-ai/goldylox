//! Core cache configuration builder implementation
//!
//! This module provides the core builder struct, constructor, and build method
//! for creating cache configurations with a fluent API and zero-allocation construction.

use arrayvec::ArrayString;

use super::super::types::{
    AlertThresholdsConfig, AnalyzerConfig, CacheConfig, ColdTierConfig, EvictionPolicy,
    HashFunction, HotTierConfig, MonitoringConfig, SkipMapConfig, WarmTierConfig, WorkerConfig,
};

/// Cache configuration builder
#[derive(Debug, Clone)]
pub struct CacheConfigBuilder {
    pub(super) config: CacheConfig,
}

impl CacheConfigBuilder {
    /// Create new configuration builder
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            config: CacheConfig {
                hot_tier: HotTierConfig {
                    max_entries: 128,
                    enabled: true,
                    hash_function: HashFunction::AHash,
                    eviction_policy: EvictionPolicy::LRU,
                    cache_line_size: 64,
                    prefetch_distance: 2,
                    _padding: [0; 5],
                },
                warm_tier: WarmTierConfig {
                    max_entries: 8192,
                    max_size_bytes: 256 * 1024 * 1024, // 256MB default
                    entry_timeout_ns: 300_000_000_000,
                    enabled: true,
                    skip_map: SkipMapConfig {
                        max_level: 16,
                        skip_probability_x1000: 500,
                        node_pool_size: 1024,
                        _padding: [0; 1],
                    },
                    promotion_threshold: 3,
                    demotion_age_threshold_ns: 600_000_000_000,
                    concurrency_level: 16,
                    load_factor_threshold: 750,
                    _padding: [0; 4],
                },
                cold_tier: ColdTierConfig {
                    enabled: false,
                    storage_path: {
                        const EMPTY_STRING: ArrayString<256> = ArrayString::new_const();
                        EMPTY_STRING
                    },
                    max_size_bytes: 1024 * 1024 * 1024, // 1GB default
                    max_file_size: 100 * 1024 * 1024,
                    compression_level: 6,
                    auto_compact: true,
                    compact_interval_ns: 3_600_000_000_000,
                    mmap_size: 1024 * 1024 * 1024,
                    write_buffer_size: 64 * 1024,
                    _padding: [0; 2],
                },
                monitoring: MonitoringConfig {
                    enabled: true,
                    sample_interval_ns: 10_000_000_000,
                    max_history_samples: 1024,
                    enable_alerts: true,
                    enable_tracing: false,
                    alert_thresholds: AlertThresholdsConfig {
                        min_hit_rate_x1000: 70_000,
                        max_access_time_ns: 1_000_000,
                        max_memory_bytes: 100 * 1024 * 1024,
                        min_ops_per_second_x100: 10_000,
                        max_error_rate_x1000: 5_000,
                    },
                    metrics_frequency_hz: 100,
                    _padding: [0; 4],
                },
                worker: WorkerConfig {
                    enabled: true,
                    thread_pool_size: 2,
                    task_queue_capacity: 1024,
                    maintenance_interval_ns: 60_000_000_000,
                    auto_tier_management: true,
                    cpu_affinity_mask: 0,
                    priority_level: 10,
                    batch_size: 32,
                    _padding: [0; 4],
                },
                analyzer: AnalyzerConfig {
                    max_tracked_keys: 10_000,
                    frequency_decay_constant: 1_000_000_000.0, // 1 second
                    recency_half_life: 300_000_000_000.0,      // 5 minutes
                    cleanup_age_threshold_ns: 3_600_000_000_000, // 1 hour
                    cleanup_interval: 1000,
                    time_bucket_count: 60, // 1 minute of buckets at 1 second each
                    time_bucket_duration_ns: 1_000_000_000, // 1 second
                    pattern_analysis_window: 100,
                },
                version: 1,
            },
        }
    }

    /// Build the configuration (zero allocation)
    #[inline(always)]
    pub const fn build(self) -> CacheConfig {
        self.config
    }
}

impl Default for CacheConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
