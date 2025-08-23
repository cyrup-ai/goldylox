//! Advanced multi-tier cache traits with sophisticated functionality
//!
//! This module provides production-quality cache traits with Generic Associated Types,
//! Higher-Ranked Trait Bounds, SIMD optimization, ML integration, and zero-copy operations.

// Core module declarations
pub mod cache_entry; // Domain model with CacheEntry<K,V> wrapper
pub mod core; // Advanced CacheKey, CacheValue, CacheTier, EvictionPolicy traits
pub mod entry_and_stats; // Cache entry and statistics traits with zero-copy monitoring
pub mod error; // Sophisticated error handling with recovery strategies
pub mod impls; // Concrete implementations for standard types
pub mod metadata; // Value metadata and serialization contexts
pub mod policy; // Eviction policies with ML optimization and HRTBs
pub mod structures; // Data structure definitions
pub mod supporting_types; // Supporting traits (HashContext, Priority, SizeEstimator)
pub mod types; // Core type definitions
pub mod types_and_enums; // Essential enumerations and value types

// Re-export public API

// Domain model
// Advanced core traits with GATs and HRTBs
pub use core::{CacheKey, CacheTier, CacheValue, EvictionPolicy};

pub use cache_entry::{
    AccessEvent, AccessTracker, CacheEntry, CacheEntryMetadata, HealthStatus, MigrationLock,
    SerializationContext, SerializationEnvelope, TierInfo, TierSerializationContext,
    TierTransition,
};
// Entry and statistics monitoring
pub use entry_and_stats::{
    AccessTracker as AccessTrackerTrait, CacheEntry as CacheEntryTrait, EvictionCandidate,
    TierStats,
};
// Sophisticated error handling
pub use error::CacheError;
// Metadata and serialization
pub use metadata::{
    CacheValueMetadata, SerializationContext as SerializationContextTrait,
    StandardSerializationContext,
};
// ML-enhanced eviction policies
pub use policy::{
    AccessEvent as PolicyAccessEvent, AccessTracker as PolicyAccessTracker, AccessTrackingStats,
    AdvancedEvictionPolicy, EvictionCandidate as PolicyEvictionCandidate,
    EvictionPolicy as PolicyEvictionPolicy, MachineLearningModel, TrainingExample,
};
// Re-export enums from supporting_types
pub use supporting_types::{CompressionAlgorithm, HashAlgorithm, SerializationFormat};
// Supporting traits for sophisticated functionality
pub use supporting_types::{
    HashContext, Priority, SizeEstimator, ValueMetadata as ValueMetadataTrait,
};
// Essential types and enums (consolidated superset)
pub use types_and_enums::{
    AccessType,
    // Added consolidated types from former types.rs
    CacheOperationError,
    CapacityInfo,
    CompressionHint,
    ErrorCategory,
    ErrorContext,
    EvictionReason,
    MaintenanceReport,
    MonitoringMetrics,
    PatternAnalysis,
    PerformanceMetrics,
    PlacementHint,
    PolicyAction,
    PolicyDecision,
    PriorityClass,
    RecoveryHint,
    SelectionReason,
    TemporalPattern,
    TierAffinity,
    TierLocation,
    VolatilityLevel,
};

// Implementations are automatically available when traits are in scope
