//! Cache policy engine for advanced cache decisions
//!
//! This module implements sophisticated cache policies including replacement algorithms,
//! write policies, and machine learning-based prefetch prediction.

pub mod data_structures;
pub mod engine;
pub mod prefetch_prediction;
pub mod replacement_policies;
pub mod types;
pub mod write_policies;

// Re-export main types and structs
// Re-export engine implementation

// Re-export the feature-rich CachePolicyEngine
pub use engine::CachePolicyEngine;
pub use types::{
    AccessSequence, AccessType, AdaptivePolicyConfig, AlgorithmMetrics, BackgroundWriteCoordinator,
    CoalescedWrite, CoordinatorStatus, DirtyEntry, LockFreeCircularBuffer, LockFreeQueue,
    PatternType, PrefetchPredictor, PrefetchSuccessTracker, PrefetchTarget, ReplacementAlgorithm,
    ReplacementPolicies, TemporalAccess, ThroughputMetrics, WritePolicy, WritePolicyManager,
    WriteScheduler,
};
