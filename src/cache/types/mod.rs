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

// REMOVED: Backwards compatibility re-exports that hide canonical API paths
// Users must now import from canonical module paths:
// - Use types::atomic::{timestamp_nanos, AccessResult, AccessStats, AtomicCacheEntry, EntryFlags, EntryMetadata}
// - Use types::batch_operations::*
// - Use types::eviction::{CandidateAnalysis, CandidateMetadata, EvictionCandidate, EvictionSelector, SelectorConfig}
// - Use types::performance::{OperationHandle, OperationTiming, PerformanceAnalysis, PerformanceReport, PerformanceSession, PrecisionTimer}
// - Use types::results::{BatchResult, CacheError, CacheResult, ErrorCategory, HitStatus, OperationMetadata, RecoveryHint}
// - Use types::simd::{AlignedAtomicCounter, BatchOperationManager, BatchPerformanceMetrics, CacheLine, SimdHasher, SimdVectorOps}
// - Use types::statistics::{AtomicTierStats, MultiTierStatistics, TierStatistics}
// - Use cache::traits::types_and_enums::CacheOperationError directly

use std::fmt::Debug;
use std::hash::Hash;

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

/// Access path tracking for cache operations
#[derive(Debug, Clone, Default)]
pub struct AccessPath {
    /// Tiers accessed in order
    pub tiers_accessed: Vec<CacheTier>,
    /// Total access time in nanoseconds
    pub total_time_ns: u64,
    /// Cache hits by tier
    pub hits_by_tier: Vec<(CacheTier, bool)>,
}

impl AccessPath {
    /// Create new access path
    pub fn new() -> Self {
        Self::default()
    }

    /// Record tier access
    pub fn record_access(&mut self, tier: CacheTier, hit: bool, time_ns: u64) {
        self.tiers_accessed.push(tier);
        self.hits_by_tier.push((tier, hit));
        self.total_time_ns += time_ns;
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
