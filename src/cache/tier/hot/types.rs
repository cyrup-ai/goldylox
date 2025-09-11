//! Type definitions and configuration for hot tier cache
//!
//! This module contains all shared types, enums, and configuration
//! structures used throughout the hot tier cache implementation.

#![allow(dead_code)] // Hot tier - SIMD-optimized high-performance cache layer

use crate::cache::traits::types_and_enums::CacheOperationError;

/// Search result for cache operations
#[derive(Debug)]
pub struct SearchResult {
    pub slot_index: usize,
    pub found: bool,
    pub collision_count: usize,
}

/// Cache operation types for statistics
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CacheOperation {
    Get,
    Put,
    Remove,
    Evict,
    Prefetch,
}

/// Hot tier cache errors
#[derive(Debug, Clone)]
pub enum HotTierError {
    /// Cache is full
    CacheFull,
    /// Entry not found
    NotFound,
    /// Invalid slot index
    InvalidSlot,
    /// Memory limit exceeded
    MemoryLimitExceeded,
    /// SIMD operation failed
    SimdError(String),
    /// Synchronization error
    SyncError(String),
}

impl From<HotTierError> for CacheOperationError {
    fn from(error: HotTierError) -> Self {
        match error {
            HotTierError::CacheFull => {
                CacheOperationError::resource_exhausted("Hot tier cache is full")
            }
            HotTierError::NotFound => CacheOperationError::NotFound,
            HotTierError::InvalidSlot => CacheOperationError::InvalidOperation,
            HotTierError::MemoryLimitExceeded => CacheOperationError::MemoryLimitExceeded,
            HotTierError::SimdError(_) => CacheOperationError::InternalError,
            HotTierError::SyncError(_) => CacheOperationError::InternalError,
        }
    }
}

/// Cache entry state for slot management
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntryState {
    Empty,
    Occupied,
    Tombstone,
}

/// Hot tier operation result
pub type HotTierResult<T> = Result<T, HotTierError>;

/// Memory alignment utilities
pub mod alignment {
    /// Check if pointer is aligned to cache line boundary
    #[inline(always)]
    pub fn is_cache_aligned<T>(ptr: *const T) -> bool {
        (ptr as usize).is_multiple_of(64)
    }

    /// Align size to cache line boundary
    #[inline(always)]
    pub fn align_to_cache_line(size: usize) -> usize {
        (size + 63) & !63
    }

    /// Get cache line count for size
    #[inline(always)]
    pub fn cache_lines_for_size(size: usize) -> usize {
        size.div_ceil(64)
    }
}

/// SIMD operation utilities
pub mod simd {
    /// Check if SIMD is available
    #[inline(always)]
    pub fn is_available() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            std::arch::is_x86_feature_detected!("avx2")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Get SIMD vector width for current architecture
    #[inline(always)]
    pub fn vector_width() -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            if is_available() {
                32 // AVX2: 256 bits = 32 bytes
            } else {
                16 // SSE: 128 bits = 16 bytes
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            8 // Fallback
        }
    }
}

/// Performance thresholds for monitoring
pub mod thresholds {
    use std::time::Duration;

    /// Maximum acceptable access latency
    pub const MAX_ACCESS_LATENCY: Duration = Duration::from_micros(10);

    /// Minimum acceptable hit rate
    pub const MIN_HIT_RATE: f64 = 0.8;

    /// Maximum memory utilization before eviction
    pub const MAX_MEMORY_UTILIZATION: f64 = 0.9;

    /// Maximum slot utilization before resize consideration
    pub const MAX_SLOT_UTILIZATION: f64 = 0.85;

    /// Minimum eviction candidate confidence
    pub const MIN_EVICTION_CONFIDENCE: f64 = 0.6;
}

/// Constants for cache operations
pub mod constants {
    /// Default hash seed
    pub const DEFAULT_HASH_SEED: u64 = 0x517cc1b727220a95;

    /// Maximum collision count before rehash
    pub const MAX_COLLISION_COUNT: usize = 8;

    /// SIMD batch size for operations
    pub const SIMD_BATCH_SIZE: usize = 4;

    /// Maximum prefetch queue size
    pub const MAX_PREFETCH_QUEUE: usize = 32;

    /// Statistics update interval
    pub const STATS_UPDATE_INTERVAL: u64 = 1000;
}
