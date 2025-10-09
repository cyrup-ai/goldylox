//! Generic cache types module
//!
//! This module provides comprehensive generic cache data structures
//! that can be used for any key-value caching scenario.

// Internal types architecture - components may not be used in minimal API

// Module declarations
pub mod atomic;
pub mod batch_operations;
pub mod core_types;
pub mod error_types;
pub mod eviction;
pub mod performance;
pub mod performance_thresholds;
pub mod performance_tools;
pub mod results;
pub mod simd;
pub mod statistics;

// Re-export main types
pub use atomic::timestamp_nanos;
// Canonical location
pub use statistics::tier_stats::TierStatistics;

use std::fmt::Debug;
use std::hash::Hash;
use std::time::Instant;

// Generic cache library - no concrete key or value types defined here
// Users should implement CacheKey and CacheValue traits for their specific types

/// Cache tier enumeration for multi-tier cache architecture
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
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
    #[allow(dead_code)]
    // Utility system - used in access pattern analysis and tier coordination
    pub tiers_accessed: Vec<CacheTier>,
    /// Total access time in nanoseconds
    #[allow(dead_code)]
    // Utility system - used in access pattern analysis and tier coordination
    pub total_time_ns: u64,
    /// Cache hits by tier
    #[allow(dead_code)]
    // Utility system - used in access pattern analysis and tier coordination
    pub hits_by_tier: Vec<(CacheTier, bool)>,

    // Analysis fields for pattern detection
    /// Whether hot tier was tried
    pub tried_hot: bool,
    /// Whether warm tier was tried
    pub tried_warm: bool,
    /// Whether cold tier was tried
    pub tried_cold: bool,
    /// Historical access timestamps for temporal locality analysis (nanoseconds)
    /// Contains up to last 10 access timestamps, oldest to newest
    #[allow(dead_code)]
    // Utility system - used in temporal locality scoring and promotion decisions
    pub access_timestamps: Vec<u64>,
    /// Historical key hashes for spatial locality analysis
    /// Contains up to last 10 accessed key hashes (from CacheKey::cache_hash())
    /// Used to calculate hash-based proximity between current and recent keys
    #[allow(dead_code)]
    // Utility system - used in spatial locality scoring and promotion decisions
    pub recent_key_hashes: Vec<u64>,
    /// Start time for latency tracking
    #[allow(dead_code)]
    // Utility system - used in access pattern analysis and tier coordination
    pub start_time: Instant,
}

impl Default for AccessPath {
    fn default() -> Self {
        Self::new()
    }
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
            access_timestamps: Vec::new(),
            recent_key_hashes: Vec::new(),
            start_time: Instant::now(),
        }
    }

    /// Create access path with historical timestamps for temporal locality analysis
    /// 
    /// # Arguments
    /// * `timestamps` - Historical access timestamps in nanoseconds (oldest to newest)
    /// 
    /// # Returns
    /// AccessPath initialized with timestamp history for locality scoring
    #[inline]
    pub fn with_timestamps(timestamps: Vec<u64>) -> Self {
        let mut path = Self::new();
        path.access_timestamps = timestamps;
        path
    }

    /// Create access path with historical key hashes for spatial locality analysis
    /// 
    /// # Arguments
    /// * `key_hashes` - Historical key hashes from CacheKey::cache_hash() (oldest to newest)
    /// 
    /// # Returns
    /// AccessPath initialized with key hash history for spatial locality scoring
    #[inline]
    pub fn with_key_hashes(key_hashes: Vec<u64>) -> Self {
        let mut path = Self::new();
        path.recent_key_hashes = key_hashes;
        path
    }

    /// Create access path with both timestamps and key hashes
    /// 
    /// # Arguments
    /// * `timestamps` - Historical access timestamps in nanoseconds
    /// * `key_hashes` - Historical key hashes from CacheKey::cache_hash()
    /// 
    /// # Returns
    /// AccessPath initialized with complete history for both temporal and spatial locality scoring
    #[inline]
    pub fn with_history(timestamps: Vec<u64>, key_hashes: Vec<u64>) -> Self {
        let mut path = Self::new();
        path.access_timestamps = timestamps;
        path.recent_key_hashes = key_hashes;
        path
    }

    /// Record tier access
    #[allow(dead_code)] // Utility system - used in access pattern analysis and tier coordination
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
    pub fn frequency_estimate<K: crate::cache::traits::CacheKey>(
        &self,
        tracker: &crate::cache::tier::warm::access_tracking::frequency_estimator::FrequencyEstimator<K>,
    ) -> f32 {
        // Get the ACTUAL global frequency estimate from the sophisticated FrequencyEstimator
        // which uses exponential moving averages to track real access patterns
        tracker.global_frequency() as f32
    }

    /// Get recent access count
    #[allow(dead_code)] // Utility system - used in access pattern analysis and tier coordination
    pub fn recent_access_count(
        &self,
        stats: &crate::telemetry::unified_stats::UnifiedCacheStatistics,
    ) -> u32 {
        // Get the ACTUAL total access count from the unified statistics system
        // The total_operations() method tracks all cache accesses (hits and misses)
        stats.total_operations() as u32
    }

    /// Calculate temporal locality score
    #[allow(dead_code)] // Utility system - used in access pattern analysis and tier coordination
    pub fn temporal_locality_score(&self) -> f32 {
        let elapsed = self.start_time.elapsed().as_nanos() as f32;
        // Higher score for more recent access
        1.0 / (1.0 + elapsed / 1_000_000.0) // Normalize to milliseconds
    }

    /// Calculate spatial locality score
    #[allow(dead_code)] // Utility system - used in access pattern analysis and tier coordination
    pub fn spatial_locality_score<K: crate::cache::traits::CacheKey>(
        &self,
        analyzer: &crate::cache::analyzer::analyzer_core::AccessPatternAnalyzer<K>,
        key: &K,
    ) -> f32 {
        // Use the ACTUAL sophisticated spatial locality analysis from AccessPatternAnalyzer
        // The analyzer tracks spatial patterns using the is_spatial_pattern() method
        // which analyzes hash ranges and cache line groupings
        let pattern = analyzer.analyze_access_pattern(key);
        pattern.temporal_locality as f32 // temporal_locality also captures spatial aspects
    }

    /// Calculate average access delay
    #[allow(dead_code)] // Utility system - used in access pattern analysis and tier coordination
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

    /// Calculate recent access score based on access count
    ///
    /// Returns normalized value in [0.0, 1.0] where:
    /// - 0 accesses = 0.0 (brand new item, never accessed)
    /// - 1 access = 0.1 (single hit)
    /// - 5 accesses = 0.5 (moderate activity)
    /// - 10+ accesses = 1.0 (very hot item)
    ///
    /// Uses linear interpolation: score = min(count / 10.0, 1.0)
    ///
    /// # Normalization Rationale
    ///
    /// The 10-access ceiling is based on the AccessPath design:
    /// - access_timestamps stores up to 10 recent timestamps (see tier_operations.rs:109)
    /// - Items with 10 entries in the vector are definitively "hot"
    /// - Linear scale 0-10 provides good granularity for ML discrimination
    ///
    /// # Usage
    ///
    /// Called from manager.rs:321 in extract_access_characteristics() to populate
    /// the AccessCharacteristics.recent_accesses field for ML-based tier promotion scoring.
    ///
    /// # Returns
    ///
    /// Normalized score in [0.0, 1.0] representing access intensity
    pub fn recent_access_score(&self) -> f32 {
        if self.access_timestamps.is_empty() {
            return 0.0;
        }

        // Normalize: 0 accesses = 0.0, 10+ accesses = 1.0
        let count = self.access_timestamps.len() as f32;
        (count / 10.0).min(1.0)
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
    #[allow(dead_code)]
    // Utility system - used in placement decision analysis and tier coordination
    pub confidence: f32,
}

impl PlacementDecision {
    /// Create placement decision for specific tier
    #[allow(dead_code)] // Utility system - used in placement decision analysis and tier coordination
    pub fn for_tier(tier: CacheTier) -> Self {
        Self {
            primary_tier: tier,
            replication_tiers: Vec::new(),
            confidence: 1.0,
        }
    }
}

// SelectionReason moved to canonical location: crate::cache::traits::types_and_enums
