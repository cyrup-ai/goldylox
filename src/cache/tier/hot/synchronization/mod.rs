//! Atomic coordination with memory ordering for hot tier cache
//!
//! This module provides thread-safe synchronization primitives and atomic
//! operations for coordinating access to the hot tier cache.

pub mod atomic_stats;
pub mod coordination;
pub mod operation_result;
pub mod simd_hash;
pub mod simd_lru;
pub mod timing;

// Re-export main types for convenience
pub use atomic_stats::{AtomicTierStats, TierStatistics};
pub use coordination::{CoordinationState, ReadGuard, WriteGuard};
pub use operation_result::OperationResult;
pub use simd_hash::SimdHashState;
pub use simd_lru::SimdLruTracker;
pub use timing::{timestamp, PrecisionTimer};
