//! Eviction policy implementations
//!
//! This module implements specific eviction algorithms including LRU, LFU,
//! ARC, and machine learning-based policies with SIMD optimization.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use super::engine::EvictionEngine;
use super::types::EvictionCandidate;
use crate::cache::tier::hot::memory_pool::{MemoryPool, SlotMetadata};
use crate::cache::tier::hot::synchronization::SimdLruTracker;
use crate::cache::traits::types_and_enums::SelectionReason;
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey + Default, V: CacheValue> EvictionEngine<K, V> {
    /// Find LRU eviction candidate using SIMD parallel search
    pub fn find_lru_candidate(
        &self,
        metadata: &[SlotMetadata],
        lru_tracker: &SimdLruTracker,
        memory_pool: &MemoryPool<K, V>,
    ) -> Option<EvictionCandidate<K, V>> {
        #[cfg(target_arch = "x86_64")]
        {
            let mut oldest_time = u64::MAX;
            let mut oldest_slot = 0;

            // SIMD parallel search for oldest timestamp
            for chunk_start in (0..256).step_by(4) {
                unsafe {
                    // Load 4 timestamps
                    let timestamps = [
                        lru_tracker.timestamps[chunk_start],
                        lru_tracker.timestamps[chunk_start + 1],
                        lru_tracker.timestamps[chunk_start + 2],
                        lru_tracker.timestamps[chunk_start + 3],
                    ];

                    let times_vec = _mm256_loadu_si256(timestamps.as_ptr() as *const __m256i);
                    let current_min = _mm256_set1_epi64x(oldest_time as i64);

                    // Find minimum (oldest) timestamp
                    let comparison = _mm256_cmpgt_epi64(current_min, times_vec);
                    let mask = _mm256_movemask_epi8(comparison);

                    // Check each slot in this chunk
                    for i in 0..4 {
                        if (mask >> (i * 8)) & 0xFF != 0 {
                            let slot_idx = chunk_start + i;
                            if metadata[slot_idx].is_occupied() && timestamps[i] < oldest_time {
                                oldest_time = timestamps[i];
                                oldest_slot = slot_idx;
                            }
                        }
                    }
                }
            }

            if oldest_time != u64::MAX {
                memory_pool.get_slot(oldest_slot).map(|slot| {
                    EvictionCandidate::from_slot_index(
                        oldest_slot,
                        slot.key.clone(),
                        1.0 / (oldest_time as f64 + 1.0),
                        SelectionReason::LeastRecentlyUsed,
                    )
                })
            } else {
                None
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            let mut oldest_time = u64::MAX;
            let mut oldest_slot = 0;

            // Fallback linear search
            let capacity = metadata.len();
            for (slot_idx, slot_metadata) in metadata.iter().enumerate().take(capacity) {
                if slot_metadata.is_occupied() && lru_tracker.timestamps[slot_idx] < oldest_time {
                    oldest_time = lru_tracker.timestamps[slot_idx];
                    oldest_slot = slot_idx;
                }
            }

            if oldest_time != u64::MAX {
                memory_pool.get_slot(oldest_slot).map(|slot| {
                    EvictionCandidate::from_slot_index(
                        oldest_slot,
                        slot.key.clone(),
                        1.0 / (oldest_time as f64 + 1.0),
                        SelectionReason::LeastRecentlyUsed,
                    )
                })
            } else {
                None
            }
        }
    }

    /// Find LFU eviction candidate
    pub fn find_lfu_candidate(
        &self,
        metadata: &[SlotMetadata],
        memory_pool: &MemoryPool<K, V>,
    ) -> Option<EvictionCandidate<K, V>> {
        let mut lowest_count = u8::MAX;
        let mut lowest_slot = 0;

        for (slot_idx, meta) in metadata.iter().enumerate() {
            if meta.is_occupied() && meta.access_count < lowest_count {
                lowest_count = meta.access_count;
                lowest_slot = slot_idx;
            }
        }

        if lowest_count != u8::MAX {
            memory_pool.get_slot(lowest_slot).map(|slot| {
                crate::cache::types::eviction::candidate::EvictionCandidate::from_slot_index(
                    lowest_slot,
                    slot.key.clone(),
                    lowest_count as f64, // LFU score based on access count
                    crate::cache::traits::types_and_enums::SelectionReason::LeastFrequentlyUsed,
                )
            })
        } else {
            None
        }
    }

    /// Find ARC (Adaptive Replacement Cache) eviction candidate
    pub fn find_arc_candidate(
        &self,
        metadata: &[SlotMetadata],
        lru_tracker: &SimdLruTracker,
        current_time_ns: u64,
        memory_pool: &MemoryPool<K, V>,
    ) -> Option<EvictionCandidate<K, V>> {
        let mut best_candidate: Option<EvictionCandidate<K, V>> = None;
        let mut best_score = f64::NEG_INFINITY;

        for (slot_idx, meta) in metadata.iter().enumerate() {
            if meta.is_occupied() {
                // ARC score combines recency and frequency
                let age = current_time_ns - lru_tracker.timestamps[slot_idx];
                let recency_score = 1.0 / (age as f64 + 1.0);
                let frequency_score = meta.access_count as f64;

                // Adaptive weighting based on cache performance
                let recency_weight = if self.performance_metrics.hit_rate_improvement > 0.0 {
                    0.6
                } else {
                    0.4
                };
                let frequency_weight = 1.0 - recency_weight;

                let score = recency_weight * recency_score + frequency_weight * frequency_score;

                if score > best_score {
                    best_score = score;
                    if let Some(slot) = memory_pool.get_slot(slot_idx) {
                        best_candidate = Some(crate::cache::types::eviction::candidate::EvictionCandidate::from_slot_index(
                            slot_idx,
                            slot.key.clone(),
                            score,
                            crate::cache::traits::types_and_enums::SelectionReason::AdaptiveReplacementCache,
                        ));
                    }
                }
            }
        }

        best_candidate
    }
}
