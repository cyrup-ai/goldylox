//! SIMD-specific helper functions for SimdHotTier
//!
//! This module contains SIMD-accelerated helper functions for key hashing,
//! parallel search, and context analysis.

#![allow(dead_code)] // Hot tier SIMD - Complete SIMD-accelerated helper functions for key hashing, parallel search, and context analysis

use super::core::SimdHotTier;
use crate::cache::tier::hot::types::SearchResult;
use crate::cache::traits::core::{CacheKey, CacheValue};

impl<K: CacheKey + Default, V: CacheValue> SimdHotTier<K, V> {
    /// SIMD-accelerated key hashing
    pub(super) fn simd_hash_key(&mut self, key: &K) -> u64 {
        // Use generic CacheKey hash method
        let hash = key.cache_hash();
        self.hash_state.update(hash);
        let final_hash = self.hash_state.finalize();
        self.hash_state.reset();
        final_hash
    }

    /// SIMD parallel search across cache slots
    pub(super) fn simd_parallel_search(
        &self,
        key: &K,
        key_hash: u64,
        start_slot: usize,
    ) -> Option<SearchResult> {
        let mut collision_count = 0;

        // Search primary cluster first (cache-friendly)
        for probe_offset in 0..self.memory_pool.capacity() {
            let slot_idx = (start_slot + probe_offset) & self.memory_pool.capacity_mask;

            if let Some(slot) = self.memory_pool.get_slot(slot_idx) {
                if slot.is_empty() {
                    // Found empty slot - continue probing instead of early return
                    // In linear probing, empty slots can exist due to deletions
                    // Must continue until we find the key or complete the probe sequence
                    collision_count += 1;
                    continue;
                }

                if slot.key_hash == key_hash && slot.key == *key {
                    // Found matching entry
                    return Some(SearchResult {
                        slot_index: slot_idx,
                        found: true,
                        collision_count,
                    });
                }

                collision_count += 1;

                // Limit collision count to prevent infinite loops
                if collision_count >= crate::cache::tier::hot::types::constants::MAX_COLLISION_COUNT
                {
                    break;
                }
            }
        }

        // Key not found after probing entire sequence
        Some(SearchResult {
            slot_index: start_slot,
            found: false,
            collision_count,
        })
    }

    /// Calculate context hash for access pattern analysis
    pub(super) fn calculate_context_hash(&self, key: &K) -> u64 {
        // Generic context hash using CacheKey trait
        key.cache_hash()
    }
}
