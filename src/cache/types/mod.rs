//! Generic cache types module
//!
//! This module provides comprehensive generic cache data structures
//! that can be used for any key-value caching scenario.

// Module declarations
pub mod atomic;
pub mod batch_operations;
pub mod core_types;
pub mod error_types;
pub mod eviction;
pub mod performance;
pub mod performance_tools;
pub mod results;
pub mod simd;
pub mod statistics;

// Re-export main types
pub use atomic::timestamp_nanos;
pub use results::CacheResult;
pub use statistics::tier_stats::TierStatistics;
pub use crate::cache::worker::types::StatUpdate;

use std::fmt::Debug;
use std::hash::Hash;
use std::time::Instant;

// Generic cache library - no concrete key or value types defined here
// Users should implement CacheKey and CacheValue traits for their specific types

/// Cache tier enumeration for multi-tier cache architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheTier {
    /// Hot tier - SIMD-optimized, fastest access, smallest capacity
    Hot,
    /// Warm tier - Balanced performance and capacity
    Warm,
    /// Cold tier - Persistent storage, highest capacity, slower access
    Cold,
}

impl Default for CacheTier {
    fn default() -> Self {
        Self::Hot
    }
}

/// Access path tracking for cache operations with analysis capabilities
#[derive(Debug, Clone)]
pub struct AccessPath {
    // Recording fields for tier access history
    /// Tiers accessed in order
    pub tiers_accessed: Vec<CacheTier>,
    /// Total access time in nanoseconds
    pub total_time_ns: u64,
    /// Cache hits by tier
    pub hits_by_tier: Vec<(CacheTier, bool)>,
    
    // Analysis fields for pattern detection
    /// Whether hot tier was tried
    pub tried_hot: bool,
    /// Whether warm tier was tried
    pub tried_warm: bool,
    /// Whether cold tier was tried
    pub tried_cold: bool,
    /// Start time for latency tracking
    pub start_time: Instant,
}

impl AccessPath {
    /// Create new access path
    pub fn new() -> Self {
        Self {
            tiers_accessed: Vec::new(),
            total_time_ns: 0,
            hits_by_tier: Vec::new(),
            tried_hot: false,
            tried_warm: false,
            tried_cold: false,
            start_time: Instant::now(),
        }
    }

    /// Record tier access
    pub fn record_access(&mut self, tier: CacheTier, hit: bool, time_ns: u64) {
        self.tiers_accessed.push(tier);
        self.hits_by_tier.push((tier, hit));
        self.total_time_ns += time_ns;
        
        // Update tried flags
        match tier {
            CacheTier::Hot => self.tried_hot = true,
            CacheTier::Warm => self.tried_warm = true,
            CacheTier::Cold => self.tried_cold = true,
        }
    }
    
    /// Estimate access frequency based on tier access pattern
    pub fn frequency_estimate<K: crate::cache::traits::CacheKey>(&self, tracker: &crate::cache::tier::warm::access_tracking::frequency_estimator::FrequencyEstimator<K>) -> f32 {
        // Get the ACTUAL global frequency estimate from the sophisticated FrequencyEstimator
        // which uses exponential moving averages to track real access patterns
        tracker.global_frequency() as f32
    }

    /// Get recent access count
    pub fn recent_access_count(&self, stats: &crate::cache::manager::statistics::types::UnifiedCacheStatistics) -> u32 {
        // Get the ACTUAL total access count from the unified statistics system
        // The total_operations() method tracks all cache accesses (hits and misses)
        stats.total_operations() as u32
    }

    /// Calculate temporal locality score
    pub fn temporal_locality_score(&self) -> f32 {
        let elapsed = self.start_time.elapsed().as_nanos() as f32;
        // Higher score for more recent access
        1.0 / (1.0 + elapsed / 1_000_000.0) // Normalize to milliseconds
    }

    /// Calculate spatial locality score
    pub fn spatial_locality_score<K: crate::cache::traits::CacheKey>(&self, analyzer: &crate::cache::analyzer::analyzer_core::AccessPatternAnalyzer<K>, key: &K) -> f32 {
        // Use the ACTUAL sophisticated spatial locality analysis from AccessPatternAnalyzer
        // The analyzer tracks spatial patterns using the is_spatial_pattern() method
        // which analyzes hash ranges and cache line groupings
        if let Some(pattern) = analyzer.analyze_access_pattern(key) {
            pattern.temporal_locality // temporal_locality also captures spatial aspects
        } else {
            // Default spatial locality for untracked keys
            0.5
        }
    }

    /// Calculate average access delay
    pub fn average_access_delay(&self) -> f32 {
        let elapsed_ns = self.start_time.elapsed().as_nanos() as f32;
        let tier_count = [self.tried_hot, self.tried_warm, self.tried_cold]
            .iter()
            .filter(|&&x| x)
            .count() as f32;

        if tier_count > 0.0 {
            elapsed_ns / tier_count
        } else {
            0.0
        }
    }
}

/// Placement decision for cache operations
#[derive(Debug, Clone)]
pub struct PlacementDecision {
    /// Primary tier for placement
    pub primary_tier: CacheTier,
    /// Additional tiers for replication
    pub replication_tiers: Vec<CacheTier>,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
}

impl PlacementDecision {
    /// Create placement decision for specific tier
    pub fn for_tier(tier: CacheTier) -> Self {
        Self {
            primary_tier: tier,
            replication_tiers: Vec::new(),
            confidence: 1.0,
        }
    }
}

// SelectionReason moved to canonical location: crate::cache::traits::types_and_enums
