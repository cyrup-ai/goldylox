//! Core data structures for unified cache statistics
//!
//! This module defines the fundamental types used for statistics collection
//! and monitoring across all cache tiers.

use std::sync::atomic::AtomicU64;

use crossbeam_utils::{atomic::AtomicCell, CachePadded};
use crate::cache::types::statistics::tier_stats::TierStatistics;

/// Unified cache statistics across all tiers
#[derive(Debug)]
pub struct UnifiedCacheStatistics {
    /// Total cache operations
    pub total_operations: CachePadded<AtomicU64>,
    /// Overall hit rate
    pub(super) overall_hit_rate: CachePadded<AtomicCell<f64>>,
    /// Per-tier hit counts
    pub(super) hot_hits: CachePadded<AtomicU64>,
    pub(super) warm_hits: CachePadded<AtomicU64>,
    pub(super) cold_hits: CachePadded<AtomicU64>,
    /// Miss count
    pub(super) total_misses: CachePadded<AtomicU64>,
    /// Average access latency
    pub avg_access_latency_ns: CachePadded<AtomicU64>,
    /// Data promotion count
    pub(super) promotions_performed: CachePadded<AtomicU64>,
    /// Data demotion count
    pub(super) demotions_performed: CachePadded<AtomicU64>,
    /// Memory usage across tiers
    pub(super) total_memory_usage: CachePadded<AtomicU64>,
    /// Cold tier specific statistics
    pub cold_tier_stats: TierStatistics,
}

// TierStatistics moved to canonical location: crate::cache::types::statistics::tier_stats::TierStatistics

/// Overall cache performance metrics
#[derive(Debug, Clone)]
pub struct CachePerformanceMetrics {
    /// Overall hit rate across all tiers
    pub overall_hit_rate: f64,
    /// Total operations processed
    pub total_operations: u64,
    /// Average access latency in nanoseconds
    pub avg_access_latency_ns: u64,
    /// Total memory usage across all tiers
    pub total_memory_usage_bytes: u64,
    /// Promotion/demotion activity
    pub promotions_performed: u64,
    pub demotions_performed: u64,
    /// Per-tier breakdown
    pub hot_tier: TierStatistics,
    pub warm_tier: TierStatistics,
    pub cold_tier: TierStatistics,
}

/// Statistics collection configuration
#[derive(Debug, Clone)]
pub struct StatisticsConfig {
    /// Enable detailed per-tier statistics
    pub enable_detailed_stats: bool,
    /// Statistics collection interval in nanoseconds
    pub collection_interval_ns: u64,
    /// Maximum statistics history to retain
    pub max_history_entries: usize,
    /// Enable performance trend analysis
    pub enable_trend_analysis: bool,
}

impl Default for StatisticsConfig {
    fn default() -> Self {
        Self {
            enable_detailed_stats: true,
            collection_interval_ns: 1_000_000_000, // 1 second
            max_history_entries: 1000,
            enable_trend_analysis: true,
        }
    }
}
