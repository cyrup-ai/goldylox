//! Hardware-specific prefetch operations
//!
//! This module provides hardware-specific prefetch instructions and
//! optimizations for different CPU architectures.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use super::super::memory_pool::CacheSlot;
use crate::cache::traits::{CacheKey, CacheValue};

/// Hardware prefetch operations
pub struct HardwarePrefetcher;

impl HardwarePrefetcher {
    /// Prefetch cache lines for upcoming access
    #[inline(always)]
    pub fn prefetch_for_access<K: CacheKey, V: CacheValue>(
        key: &K,
        entries: &[CacheSlot<K, V>; 256],
    ) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            // Calculate likely cache slot using generic key hash
            let hash_context = key.hash_context();
            let key_hash = key
                .fast_hash(&hash_context)
                .wrapping_mul(0x9e3779b97f4a7c15);
            let slot_idx = (key_hash as usize) & 255;

            // Prefetch cache lines
            let slot_ptr = &entries[slot_idx] as *const CacheSlot<K, V>;

            // Hardware prefetch hint (L1 cache)
            _mm_prefetch::<{ _MM_HINT_T0 }>(slot_ptr as *const i8);

            // Prefetch adjacent slots for spatial locality
            if slot_idx + 1 < 256 {
                let next_slot_ptr = &entries[slot_idx + 1] as *const CacheSlot<K, V>;
                _mm_prefetch::<{ _MM_HINT_T1 }>(next_slot_ptr as *const i8);
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            // No-op on non-x86_64 platforms
            let _ = (key, entries);
        }
    }

    /// Prefetch multiple slots for batch operations
    #[inline(always)]
    pub fn prefetch_batch<K: CacheKey, V: CacheValue>(
        slot_indices: &[usize],
        entries: &[CacheSlot<K, V>; 256],
    ) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            for &slot_idx in slot_indices.iter().take(8) {
                // Limit to 8 prefetches
                if slot_idx < 256 {
                    let slot_ptr = &entries[slot_idx] as *const CacheSlot;
                    _mm_prefetch::<{ _MM_HINT_T0 }>(slot_ptr as *const i8);
                }
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            let _ = (slot_indices, entries);
        }
    }

    /// Prefetch with specific cache level hint
    #[inline(always)]
    pub fn prefetch_with_hint(ptr: *const u8, hint: PrefetchHint) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            match hint {
                PrefetchHint::L1Cache => _mm_prefetch::<{ _MM_HINT_T0 }>(ptr as *const i8),
                PrefetchHint::L2Cache => _mm_prefetch::<{ _MM_HINT_T1 }>(ptr as *const i8),
                PrefetchHint::L3Cache => _mm_prefetch::<{ _MM_HINT_T2 }>(ptr as *const i8),
                PrefetchHint::NonTemporal => _mm_prefetch::<{ _MM_HINT_NTA }>(ptr as *const i8),
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            let _ = (ptr, hint);
        }
    }

    /// Prefetch cache line containing specific address
    #[inline(always)]
    pub fn prefetch_address(addr: *const u8) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            _mm_prefetch::<{ _MM_HINT_T0 }>(addr as *const i8);
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            let _ = addr;
        }
    }

    /// Prefetch sequential cache lines
    #[inline(always)]
    pub fn prefetch_sequential(base_addr: *const u8, count: usize, stride: usize) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            for i in 0..count.min(8) {
                // Limit prefetch distance
                let addr = base_addr.add(i * stride);
                _mm_prefetch::<{ _MM_HINT_T0 }>(addr as *const i8);
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            let _ = (base_addr, count, stride);
        }
    }

    /// Check if hardware prefetch is supported
    pub fn is_supported() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            // x86_64 always supports prefetch instructions
            true
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Get optimal prefetch distance for current hardware
    pub fn optimal_prefetch_distance() -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            // Typical optimal distance for x86_64
            8
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            1
        }
    }

    /// Prefetch with adaptive distance based on access pattern
    #[inline(always)]
    pub fn prefetch_adaptive(
        base_addr: *const u8,
        access_pattern: AccessPatternHint,
        distance: usize,
    ) {
        let actual_distance = match access_pattern {
            AccessPatternHint::Sequential => distance,
            AccessPatternHint::Random => distance / 2,
            AccessPatternHint::Strided => distance * 2,
        };

        Self::prefetch_sequential(base_addr, actual_distance, 64); // 64-byte cache line
    }
}

/// Prefetch hint for cache level targeting
#[derive(Debug, Clone, Copy)]
pub enum PrefetchHint {
    /// Prefetch to L1 cache (highest priority)
    L1Cache,
    /// Prefetch to L2 cache
    L2Cache,
    /// Prefetch to L3 cache
    L3Cache,
    /// Non-temporal prefetch (bypass cache)
    NonTemporal,
}

/// Access pattern hint for adaptive prefetching
#[derive(Debug, Clone, Copy)]
pub enum AccessPatternHint {
    /// Sequential access pattern
    Sequential,
    /// Random access pattern
    Random,
    /// Strided access pattern
    Strided,
}

/// Hardware prefetch statistics
#[derive(Debug, Default)]
pub struct HardwarePrefetchStats {
    pub prefetch_requests: u64,
    pub l1_prefetches: u64,
    pub l2_prefetches: u64,
    pub l3_prefetches: u64,
    pub non_temporal_prefetches: u64,
}

impl HardwarePrefetchStats {
    /// Record a prefetch operation
    pub fn record_prefetch(&mut self, hint: PrefetchHint) {
        self.prefetch_requests += 1;

        match hint {
            PrefetchHint::L1Cache => self.l1_prefetches += 1,
            PrefetchHint::L2Cache => self.l2_prefetches += 1,
            PrefetchHint::L3Cache => self.l3_prefetches += 1,
            PrefetchHint::NonTemporal => self.non_temporal_prefetches += 1,
        }
    }

    /// Get prefetch efficiency ratio
    pub fn efficiency_ratio(&self) -> f64 {
        if self.prefetch_requests == 0 {
            return 0.0;
        }

        // Higher weight for L1 prefetches as they're more effective
        let weighted_prefetches = self.l1_prefetches * 4
            + self.l2_prefetches * 2
            + self.l3_prefetches
            + self.non_temporal_prefetches;

        weighted_prefetches as f64 / (self.prefetch_requests * 4) as f64
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
