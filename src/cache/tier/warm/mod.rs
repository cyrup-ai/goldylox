//! Lock-free warm tier cache with crossbeam skiplist for concurrent shared access
//!
//! This module implements the L2 Warm Tier cache using advanced lock-free data structures,
//! concurrent access patterns, and sophisticated eviction algorithms for high-throughput sharing.

pub mod access_tracking;
pub mod atomic_float;
pub mod builder;
pub mod config;
pub mod coordination;
pub mod core;
pub mod data_structures;
pub mod error;
pub mod eviction;
pub mod global_api;
pub mod maintenance;
pub mod memory_monitor_enum;
pub mod memory_monitor_trait;
pub mod metrics;
pub mod monitoring;
pub mod noop_monitor;
pub mod timing;

// Re-export the main API for easy access
// Re-export core types
pub use core::{WarmCacheEntry, WarmCacheKey, WarmEntryMetadata};

// Re-export access tracking types
pub use access_tracking::{
    AccessContext, AccessRecord, AccessType, ConcurrentAccessTracker, FrequencyEstimator,
    PatternType, TemporalPatternClassifier,
};
pub use builder::WarmTierBuilder;
pub use data_structures::{
    AnalysisDepth, BackgroundConfig, ConsistencyLevel, LockFreeWarmTier, MaintenanceTask,
    ModelComplexity, OptimizationLevel, PerformanceConfig, PerformanceMetrics, PressureConfig,
    SyncDirection, TrackingConfig, ValidationLevel, WarmTierConfig,
};
pub use error::{DegradationMode, RetryConfig, WarmTierInitError};
// Re-export eviction types
pub use eviction::{
    ArcEvictionState, ConcurrentEvictionPolicy, ConcurrentLfuTracker, ConcurrentLruTracker,
    EvictionPolicy, EvictionPolicyType, MachineLearningEvictionPolicy, PolicyPerformanceMetrics,
};
pub use global_api::{
    check_warm_tier_alerts, cleanup_expired_entries, force_eviction, get_cache_size,
    get_frequently_accessed_keys, get_idle_keys, get_memory_pressure, get_memory_usage,
    get_warm_tier_keys, init_warm_tier, insert_demoted, insert_promoted, is_initialized,
    process_background_maintenance, shutdown_warm_tier, warm_get, warm_put, warm_remove,
};
// Re-export monitoring types
pub use monitoring::{
    AtomicTierStats, MemoryAlert, MemoryPressureMonitor, MonitoringTask, TierStatsSnapshot,
};

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

// REMOVED: All convenience functions with hardcoded String types.
// These functions masked the generic nature of the cache system and forced String defaults.
// Users must now call the underlying generic functions directly with explicit type parameters.
//
// Removed functions:
// - init_default() -> use init_warm_tier::<K, V>(config) directly
// - contains_key<K,V>() -> use warm_get::<K, V>().is_some() directly
// - get_utilization() -> use get_memory_pressure::<K, V>() directly
// - get_cache_stats() -> use get_warm_tier_stats::<K, V>() directly
// - perform_maintenance() -> use process_background_maintenance::<K, V>() directly
// - is_under_pressure() -> use get_memory_pressure::<K, V>() directly
// - get_health_score() -> compose from get_warm_tier_stats::<K, V>() and get_memory_pressure::<K, V>()
// - compact_cache() -> use cleanup_expired_entries::<K, V>() directly
// - get_efficiency_metrics() -> use get_warm_tier_stats::<K, V>() directly

// REMOVED: estimate_effectiveness() - used removed get_health_score() function.
// Users must implement effectiveness estimation using generic get_warm_tier_stats::<K, V>()
// and get_memory_pressure::<K, V>() functions directly.

/// Access pattern types for effectiveness estimation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPattern {
    /// Sequential access pattern
    Sequential,
    /// Random access pattern  
    Random,
    /// Temporal locality pattern
    Temporal,
    /// Spatial locality pattern
    Spatial,
    /// Burst access pattern
    Burst,
    /// Working set pattern
    Working,
}

// REMOVED: warm_cache_with_predictions() and tune_for_workload() - assumed String types.
// These placeholder functions made String type assumptions.
// Users must implement cache warming and tuning strategies using the generic APIs directly.

/// Workload characteristics for cache tuning
#[derive(Debug, Clone)]
pub struct WorkloadCharacteristics {
    /// Average request rate (requests per second)
    pub avg_request_rate: f64,
    /// Peak request rate
    pub peak_request_rate: f64,
    /// Working set size estimate
    pub working_set_size: usize,
    /// Access pattern distribution
    pub pattern_distribution: Vec<(AccessPattern, f64)>,
    /// Average item size in bytes
    pub avg_item_size: usize,
    /// Temporal locality strength (0.0-1.0)
    pub temporal_locality: f64,
    /// Spatial locality strength (0.0-1.0)
    pub spatial_locality: f64,
}

impl WorkloadCharacteristics {
    /// Create workload characteristics for read-heavy workloads
    pub fn read_heavy() -> Self {
        Self {
            avg_request_rate: 1000.0,
            peak_request_rate: 5000.0,
            working_set_size: 10000,
            pattern_distribution: vec![
                (AccessPattern::Temporal, 0.6),
                (AccessPattern::Working, 0.3),
                (AccessPattern::Random, 0.1),
            ],
            avg_item_size: 1024,
            temporal_locality: 0.8,
            spatial_locality: 0.4,
        }
    }

    /// Create workload characteristics for write-heavy workloads
    pub fn write_heavy() -> Self {
        Self {
            avg_request_rate: 500.0,
            peak_request_rate: 2000.0,
            working_set_size: 5000,
            pattern_distribution: vec![
                (AccessPattern::Sequential, 0.4),
                (AccessPattern::Burst, 0.3),
                (AccessPattern::Random, 0.3),
            ],
            avg_item_size: 2048,
            temporal_locality: 0.5,
            spatial_locality: 0.6,
        }
    }

    /// Create workload characteristics for mixed workloads
    pub fn mixed() -> Self {
        Self {
            avg_request_rate: 750.0,
            peak_request_rate: 3000.0,
            working_set_size: 7500,
            pattern_distribution: vec![
                (AccessPattern::Temporal, 0.4),
                (AccessPattern::Sequential, 0.2),
                (AccessPattern::Working, 0.2),
                (AccessPattern::Random, 0.2),
            ],
            avg_item_size: 1536,
            temporal_locality: 0.65,
            spatial_locality: 0.5,
        }
    }
}
