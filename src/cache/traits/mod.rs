//! Advanced multi-tier cache traits with sophisticated functionality
//!
//! This module provides production-quality cache traits with Generic Associated Types,
//! Higher-Ranked Trait Bounds, SIMD optimization, ML integration, and zero-copy operations.

// Core module declarations (crate private)
pub(crate) mod cache_entry; // Domain model with CacheEntry<K,V> wrapper
pub(crate) mod core; // Advanced CacheKey, CacheValue, CacheTier, EvictionPolicy traits
pub(crate) mod entry_and_stats; // Cache entry and statistics traits with zero-copy monitoring
pub(crate) mod error; // Sophisticated error handling with recovery strategies
pub(crate) mod impls; // Concrete implementations for standard types
pub(crate) mod metadata; // Value metadata and serialization contexts
pub(crate) mod policy; // Eviction policies with ML optimization and HRTBs
pub(crate) mod structures; // Data structure definitions
pub(crate) mod supporting_types; // Supporting traits (HashContext, Priority, SizeEstimator)
pub(crate) mod types; // Core type definitions
pub(crate) mod types_and_enums; // Essential enumerations and value types

// Re-export API for crate-internal use only

// Domain model
// Advanced core traits with GATs and HRTBs
pub(crate) use core::{CacheKey, CacheValue};

pub(crate) use cache_entry::CacheEntry;
// Entry and statistics monitoring
pub(crate) use entry_and_stats::{
    TierStats,
};
// Sophisticated error handling
// Metadata and serialization
// ML-enhanced eviction policies
// Re-export enums from supporting_types
pub(crate) use supporting_types::CompressionAlgorithm;
// Supporting traits for sophisticated functionality
// Note: HashContext and Priority are not used via traits module - removed unused re-exports
// Essential types and enums (consolidated superset)
pub(crate) use types_and_enums::{
    AccessType,
    // Added consolidated types from former types.rs
    CacheOperationError,
    TemporalPattern,
    TierLocation,
};

// Implementations are automatically available when traits are in scope
