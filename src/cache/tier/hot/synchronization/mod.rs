#![allow(dead_code)]
// Hot tier synchronization - Complete synchronization library with atomic coordination, memory ordering, and thread-safe primitives

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

pub use coordination::{CoordinationState, ReadGuard, WriteGuard};
pub use simd_hash::SimdHashState;
pub use simd_lru::SimdLruTracker;
// PrecisionTimer is now available from crate::cache::types::performance::timer::PrecisionTimer
