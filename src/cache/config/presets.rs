//! Ultra-fast predefined configuration presets
//!
//! This module provides compile-time constant configuration presets
//! for common use cases with optimal performance characteristics.

use arrayvec::ArrayString;

use super::builder::CacheConfigBuilder;
use super::types::{
    AlertThresholdsConfig, AnalyzerConfig, CacheConfig, ColdTierConfig, EvictionPolicy,
    HashFunction, HotTierConfig, MonitoringConfig, SkipMapConfig, WarmTierConfig, WorkerConfig,
};

/// Ultra-fast predefined configuration presets (compile-time constants)
pub struct ConfigPresets;

impl ConfigPresets {
    /// High-performance configuration for speed-critical applications
    #[inline(always)]
    pub const fn high_performance() -> CacheConfig {
        CacheConfigBuilder::new()
            .hot_tier_capacity(256) // Power of 2
            .warm_tier_capacity(16384) // Power of 2
            .hash_function(HashFunction::XxHash)
            .eviction_policy(EvictionPolicy::LRU2)
            .monitoring_interval_ns(5_000_000_000) // 5 seconds
            .worker_threads(4)
            .build()
    }

    /// Memory-efficient configuration for memory-constrained environments
    #[inline(always)]
    pub const fn memory_efficient() -> CacheConfig {
        CacheConfigBuilder::new()
            .hot_tier_capacity(64) // Power of 2
            .warm_tier_capacity(1024) // Power of 2
            .cold_tier_compression(9)
            .monitoring_interval_ns(30_000_000_000) // 30 seconds
            .worker_threads(1)
            .build()
    }

    /// Balanced configuration for general use
    #[inline(always)]
    pub const fn balanced() -> CacheConfig {
        CacheConfig {
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
                max_size_bytes: 128 * 1024 * 1024, // 128MB for high performance
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
                max_size_bytes: 512 * 1024 * 1024, // 512MB for high performance
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
        }
    }

    /// Development configuration with extensive monitoring
    #[inline(always)]
    pub const fn development() -> CacheConfig {
        CacheConfigBuilder::new()
            .hot_tier_capacity(128) // Power of 2
            .warm_tier_capacity(2048) // Power of 2
            .monitoring_enabled(true)
            .monitoring_interval_ns(1_000_000_000) // 1 second
            .alert_thresholds(AlertThresholdsConfig {
                min_hit_rate_x1000: 50_000,         // 50%
                max_access_time_ns: 5_000_000,      // 5ms
                max_memory_bytes: 50 * 1024 * 1024, // 50MB
                min_ops_per_second_x100: 1_000,     // 10.0
                max_error_rate_x1000: 10_000,       // 10%
            })
            .build()
    }

    /// Production configuration with optimized settings
    #[inline(always)]
    pub const fn production() -> CacheConfig {
        CacheConfigBuilder::new()
            .hot_tier_capacity(512) // Power of 2
            .warm_tier_capacity(32768) // Power of 2
            .hash_function(HashFunction::XxHash)
            .eviction_policy(EvictionPolicy::ARC)
            .monitoring_interval_ns(15_000_000_000) // 15 seconds
            .worker_threads(8)
            .auto_tier_management(true)
            .alert_thresholds(AlertThresholdsConfig {
                min_hit_rate_x1000: 80_000,          // 80%
                max_access_time_ns: 500_000,         // 0.5ms
                max_memory_bytes: 500 * 1024 * 1024, // 500MB
                min_ops_per_second_x100: 50_000,     // 500.0
                max_error_rate_x1000: 1_000,         // 1%
            })
            .build()
    }

    /// Testing configuration with minimal overhead
    #[inline(always)]
    pub const fn testing() -> CacheConfig {
        CacheConfigBuilder::new()
            .hot_tier_capacity(32) // Power of 2
            .warm_tier_capacity(256) // Power of 2
            .monitoring_enabled(false)
            .worker_enabled(false)
            .build()
    }

    /// Embedded configuration for resource-constrained environments
    #[inline(always)]
    pub const fn embedded() -> CacheConfig {
        CacheConfigBuilder::new()
            .hot_tier_capacity(16) // Power of 2
            .warm_tier_capacity(128) // Power of 2
            .monitoring_enabled(false)
            .worker_threads(1)
            .analyzer_max_keys(1000)
            .build()
    }

    /// Debug configuration with extensive logging and monitoring
    #[inline(always)]
    pub const fn debug() -> CacheConfig {
        CacheConfigBuilder::new()
            .hot_tier_capacity(64) // Power of 2
            .warm_tier_capacity(512) // Power of 2
            .monitoring_enabled(true)
            .enable_tracing(true)
            .enable_alerts(true)
            .monitoring_interval_ns(100_000_000) // 100ms
            .metrics_frequency_hz(1000) // 1kHz
            .alert_thresholds(AlertThresholdsConfig {
                min_hit_rate_x1000: 10_000,         // 10%
                max_access_time_ns: 10_000_000,     // 10ms
                max_memory_bytes: 10 * 1024 * 1024, // 10MB
                min_ops_per_second_x100: 100,       // 1.0
                max_error_rate_x1000: 50_000,       // 50%
            })
            .build()
    }
}
