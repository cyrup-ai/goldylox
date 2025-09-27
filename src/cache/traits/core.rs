//! Advanced core cache traits with sophisticated optimization features
//!
//! This module defines advanced cache traits with GATs, HRTBs, and sophisticated
//! performance optimization features for zero-cost abstractions.

#![allow(dead_code)] // Cache traits - Core trait definitions for cache system abstractions

// Internal trait definitions - may not be used in minimal API

use std::fmt::Debug;
use std::hash::Hash;
use std::time::Instant;

use super::entry_and_stats::{AccessTracker, CacheEntry, CacheError, EvictionCandidate, TierStats};
use super::supporting_types::ValueMetadata;
use super::types_and_enums::{
    AccessType, CapacityInfo, CompressionHint, EvictionReason, MaintenanceReport,
    PerformanceMetrics, PlacementHint, TierAffinity,
};

/// Advanced cache key trait with intelligent caching features
pub trait CacheKey:
    Clone
    + Send
    + Sync
    + Debug
    + Hash
    + Eq
    + Ord
    + serde::Serialize
    + serde::de::DeserializeOwned
    + 'static
{
    /// Hash context type for specialized hashing
    type HashContext: super::supporting_types::HashContext;
    /// Priority type for eviction decisions
    type Priority: super::supporting_types::Priority;
    /// Size estimator type for memory accounting
    type SizeEstimator: super::supporting_types::SizeEstimator;

    /// Size estimation for memory accounting
    fn estimated_size(&self) -> usize;

    /// Cache affinity hint for initial tier placement
    fn tier_affinity(&self) -> TierAffinity {
        TierAffinity::Auto
    }

    /// Simple cache hash for eviction and placement decisions
    fn cache_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Get hash context for advanced hashing
    fn hash_context(&self) -> Self::HashContext;

    /// Get priority for eviction decisions
    fn priority(&self) -> Self::Priority;

    /// Get size estimator for memory accounting
    fn size_estimator(&self) -> Self::SizeEstimator;

    /// Compute fast hash with context
    fn fast_hash(&self, context: &Self::HashContext) -> u64;
}

/// Advanced cache value trait with intelligent caching features
pub trait CacheValue:
    Clone + Send + Sync + Debug + serde::Serialize + serde::de::DeserializeOwned + 'static
{
    /// Metadata type for intelligent caching decisions
    type Metadata: ValueMetadata;

    /// Size estimation for memory accounting
    fn estimated_size(&self) -> usize;

    /// Check if value is expensive to recreate (affects eviction priority)
    fn is_expensive(&self) -> bool {
        true
    }

    /// Compression hint for cold tier storage
    fn compression_hint(&self) -> CompressionHint {
        CompressionHint::Auto
    }

    /// Get metadata for intelligent caching decisions
    fn metadata(&self) -> Self::Metadata;
}

/// Generic Associated Type trait for cache tier implementations
pub trait CacheTier<K: CacheKey, V: CacheValue>: Send + Sync + Debug {
    /// Statistics type with lifetime parameters (GAT)
    type Stats<'a>: TierStats + Debug + 'a
    where
        Self: 'a;

    /// Entry type with associated lifetime (GAT)
    type Entry<'a>: CacheEntry<K, V> + 'a
    where
        Self: 'a;

    /// Iterator type for cache traversal (GAT with HRTB)
    type Iter<'a>: Iterator<Item = Self::Entry<'a>> + 'a
    where
        Self: 'a;

    /// Error type for cache operations
    type Error: CacheError + Send + Sync;

    /// Get entry from cache with lifetime management
    fn get<'a>(&'a self, key: &K) -> Result<Option<Self::Entry<'a>>, Self::Error>;

    /// Put value in cache with advanced placement strategies
    fn put(&self, key: K, value: V, placement: PlacementHint) -> Result<(), Self::Error>;

    /// Remove entry with confirmation
    fn remove(&self, key: &K) -> Result<bool, Self::Error>;

    /// Get comprehensive statistics (GAT with lifetime)
    fn stats<'a>(&'a self) -> Self::Stats<'a>;

    /// Iterate over all entries (GAT with HRTB)
    fn iter<'a>(&'a self) -> Self::Iter<'a>;

    /// Clear all entries with confirmation
    fn clear(&self) -> Result<usize, Self::Error>;

    /// Get capacity information
    fn capacity(&self) -> CapacityInfo;

    /// Perform maintenance operations
    fn maintain(&self) -> Result<MaintenanceReport, Self::Error>;
}

/// Higher-Ranked Trait Bound for eviction policy implementations with advanced lifetime management
pub trait EvictionPolicy<K: CacheKey, V: CacheValue>: Send + Sync + Debug {
    /// Access pattern tracker type (GAT)
    type AccessTracker<'a>: AccessTracker<K, V> + 'a
    where
        Self: 'a;

    /// Eviction candidate type (GAT)
    type Candidate<'a>: EvictionCandidate<K, V> + 'a
    where
        Self: 'a;

    /// Select eviction candidate using HRTB for lifetime polymorphism
    fn select_candidate<'a, F>(&'a self, accessor: F) -> Option<Self::Candidate<'a>>
    where
        F: for<'b> FnOnce(&'b Self::AccessTracker<'b>) -> &'b Self::AccessTracker<'b>;

    /// Record access event with advanced pattern recognition
    fn record_access(&mut self, key: &K, access_type: AccessType, timestamp: Instant);

    /// Record eviction event for policy adaptation
    fn record_eviction(&mut self, key: &K, reason: EvictionReason, value_metadata: &V::Metadata);

    /// Get access tracker for analysis (GAT)
    fn access_tracker<'a>(&'a self) -> Self::AccessTracker<'a>;

    /// Optimize policy parameters based on access patterns
    fn optimize(&mut self, performance_metrics: &PerformanceMetrics);
}
