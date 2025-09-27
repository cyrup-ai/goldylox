//! Hot tier cache module with SIMD-optimized performance
//!
//! This module provides the L1 hot tier cache with decomposed submodules
//! for memory allocation, eviction, prefetching, and synchronization.

// Declare submodules
pub mod atomic_ops;
pub mod eviction;
pub mod memory_pool;
pub mod prefetch;
pub mod simd_tier;
pub mod synchronization;
pub mod thread_local;
pub mod types;

// Re-export key types for public API
// Re-export TierStatistics from canonical location

// PrecisionTimer is now available from crate::cache::types::performance::timer::PrecisionTimer
pub use atomic_ops::{compare_and_swap_atomic, put_if_absent_atomic, replace_atomic};
pub use thread_local::{
    cleanup_expired_entries, get_idle_keys, init_simd_hot_tier, initialize_hot_tier_system,
    insert_promoted, remove_entry, simd_hot_get, simd_hot_put, simd_hot_remove,
};
