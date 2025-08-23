//! Unified cache statistics and monitoring modules
//!
//! This module provides comprehensive statistics collection and monitoring
//! across all cache tiers with atomic operations and performance tracking.

pub mod performance_metrics;
pub mod tier_stats;
pub mod types;
pub mod unified_stats;

// REMOVED: Backwards compatibility re-exports that hide canonical API paths
// Users must now import from canonical module path:
// - Use manager::statistics::types::{CachePerformanceMetrics, StatisticsConfig, TierStatistics, UnifiedCacheStatistics}
