//! Hot tier cache module with SIMD-optimized performance
//!
//! This module provides the L1 hot tier cache with decomposed submodules
//! for memory allocation, eviction, prefetching, and synchronization.

// Declare submodules
pub mod eviction;
pub mod memory_pool;
pub mod prefetch;
pub mod simd_tier;
pub mod synchronization;
pub mod thread_local;
pub mod types;

// Re-export key types for public API
pub use eviction::{EvictionEngine, EvictionPolicy, EvictionStats};
pub use memory_pool::{CacheSlot, MemoryPool, MemoryPoolStats, SlotMetadata};
pub use prefetch::{PredictionConfidence, PrefetchPredictor, PrefetchRequest, PrefetchStats};
pub use simd_tier::SimdHotTier;
// Re-export TierStatistics from canonical location
pub use crate::cache::types::statistics::tier_stats::TierStatistics;


// PrecisionTimer is now available from crate::cache::types::performance::timer::PrecisionTimer
pub use thread_local::{
    cleanup_expired_entries, compact_hot_tier, get_frequently_accessed_keys, get_idle_keys,
    hot_tier_config, hot_tier_eviction_stats, hot_tier_memory_stats, hot_tier_prefetch_stats,
    init_simd_hot_tier, insert_promoted, process_prefetch_requests, remove_entry,
    should_optimize_hot_tier, simd_hot_get, simd_hot_put, simd_hot_remove, simd_hot_stats,
};
pub use crate::cache::config::types::HotTierConfig;
pub use types::{HotTierError, HotTierResult, SearchResult};
