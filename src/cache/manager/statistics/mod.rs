//! Unified cache statistics and monitoring modules
//!
//! This module provides comprehensive statistics collection and monitoring
//! across all cache tiers with atomic operations and performance tracking.

pub mod performance_metrics;
pub mod tier_stats;
// REMOVED: pub mod types; (canonicalized to telemetry::unified_stats)
pub mod unified_stats;

// REMOVED: Backwards compatibility re-exports that hide canonical API paths
// Users must now import from canonical module path:
// - Use telemetry::unified_stats::{CachePerformanceMetrics, StatisticsConfig, UnifiedCacheStatistics}
