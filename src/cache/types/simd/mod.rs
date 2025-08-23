//! SIMD-optimized cache operations
//!
//! This module provides SIMD-optimized data structures and operations
//! for high-performance cache management and analysis.

pub mod batch_ops;
pub mod hasher;
pub mod memory;
pub mod vectorops;

// Re-export main types for easy access
/// Type aliases for common SIMD configurations
pub use batch_ops::{
    BatchOperationManager, BatchPerformanceMetrics, GenericBatchResult, LatencyDistribution,
    ThroughputMetrics,
};
pub use hasher::SimdHasher;
pub use memory::{AlignedAtomicCounter, CacheLine};
pub use vectorops::SimdVectorOps;

// Additional type aliases and re-exports for compatibility
