//! Core cache operations for SimdHotTier
//!
//! This module contains the main cache operations including get, put, and remove
//! with SIMD-accelerated lookup and intelligent eviction.

#![allow(dead_code)] // Hot tier SIMD - Complete SIMD-accelerated cache operations with intelligent eviction and prefetch prediction

use super::core::SimdHotTier;
use crate::cache::tier::hot::synchronization::timing::timestamp;
use crate::cache::tier::hot::synchronization::{ReadGuard, WriteGuard};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::performance::timer::PrecisionTimer;

impl<K: CacheKey + Default, V: CacheValue> SimdHotTier<K, V> {
    /// Get entry from cache with SIMD-accelerated lookup
    pub fn get(&mut self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();
        let context_hash = self.calculate_context_hash(key);

        // Compute hash with SIMD acceleration before acquiring lock
        let key_hash = self.simd_hash_key(key);

        // Try to acquire read lock
        let _guard = match ReadGuard::new(&self.coordination) {
            Some(guard) => guard,
            None => {
                self.stats.record_error(timer.elapsed_ns());
                return None;
            }
        };

        // Record access for prefetch learning
        self.prefetch_predictor
            .record_access(key, timestamp::now_nanos(), context_hash);
        let primary_slot = self.memory_pool.slot_index_from_hash(key_hash);

        // Search for the key
        if let Some(search_result) = self.simd_parallel_search(key, key_hash, primary_slot)
            && search_result.found
        {
            // Cache hit - clone value first
            let value_clone =
                if let Some(slot) = self.memory_pool.get_slot(search_result.slot_index) {
                    slot.value.clone()
                } else {
                    None
                };

            if let Some(value) = value_clone {
                // Update LRU tracking
                self.lru_tracker.record_access(search_result.slot_index);

                // Update metadata
                if let Some(metadata) = self.memory_pool.get_metadata_mut(search_result.slot_index)
                {
                    metadata.record_access();
                }

                // Record hit
                self.stats.record_hit(timer.elapsed_ns());
                self.eviction_engine.record_access(
                    key.clone(),
                    search_result.slot_index,
                    true,
                    timer.elapsed_ns(),
                );

                return Some(value);
            }
        }

        // Cache miss
        self.stats.record_miss(timer.elapsed_ns());
        self.eviction_engine
            .record_access(key.clone(), 0, false, timer.elapsed_ns());
        None
    }

    /// Put entry in cache with intelligent eviction
    pub fn put(&mut self, key: K, value: V) -> Result<(), CacheOperationError> {
        let timer = PrecisionTimer::start();

        // Try to acquire write lock
        let _guard = match WriteGuard::new(&self.coordination) {
            Some(guard) => guard,
            None => {
                self.stats.record_error(timer.elapsed_ns());
                return Err(CacheOperationError::resource_exhausted("Cache busy"));
            }
        };

        // Check memory limit
        if self.memory_pool.total_memory_usage() + value.estimated_size()
            > (self.config.memory_limit_mb as usize * 1024 * 1024)
        {
            return Err(CacheOperationError::resource_exhausted(
                "Memory limit exceeded",
            ));
        }

        let key_hash = {
            let hash = key.cache_hash();
            self.hash_state.update(hash);
            let final_hash = self.hash_state.finalize();
            self.hash_state.reset();
            final_hash
        };
        let primary_slot = self.memory_pool.slot_index_from_hash(key_hash);

        // Try to find existing entry first
        let search_result = self.simd_parallel_search(&key, key_hash, primary_slot);
        if let Some(result) = search_result
            && result.found
        {
            // Update existing entry and record access
            self.lru_tracker.record_access(result.slot_index);
            self.memory_pool
                .update_at_slot(result.slot_index, value, timestamp::now_nanos());
            return Ok(());
        }

        // Try to allocate new slot
        if let Some(slot_idx) = self.memory_pool.allocate_slot() {
            self.memory_pool.insert_at_slot(
                slot_idx,
                key.clone(),
                value,
                key_hash,
                timestamp::now_nanos(),
            );
            self.lru_tracker.record_access(slot_idx);
            return Ok(());
        }

        // Need to evict entry
        let current_time = timestamp::now_nanos();
        if let Some(candidate) = self.eviction_engine.find_eviction_candidate(
            &self.memory_pool.metadata,
            &self.lru_tracker,
            current_time,
            &self.memory_pool,
        ) {
            // Evict the candidate
            let slot_idx = candidate.slot_index().unwrap_or(0); // Get slot index or use 0 as fallback
            self.memory_pool.clear_slot(slot_idx);
            self.eviction_engine
                .record_eviction(slot_idx, true, timer.elapsed_ns());

            // Insert new entry
            self.memory_pool
                .insert_at_slot(slot_idx, key, value, key_hash, current_time);
            self.lru_tracker.record_access(slot_idx);

            Ok(())
        } else {
            self.stats.record_error(timer.elapsed_ns());
            Err(CacheOperationError::resource_exhausted(
                "No eviction candidate found",
            ))
        }
    }

    /// Remove entry from cache
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();

        // Compute hash with SIMD acceleration before acquiring lock
        let key_hash = self.simd_hash_key(key);

        // Try to acquire write lock
        let _guard = match WriteGuard::new(&self.coordination) {
            Some(guard) => guard,
            None => {
                self.stats.record_error(timer.elapsed_ns());
                return None;
            }
        };
        let primary_slot = self.memory_pool.slot_index_from_hash(key_hash);

        if let Some(search_result) = self.simd_parallel_search(key, key_hash, primary_slot)
            && search_result.found
            && let Some(slot) = self.memory_pool.get_slot_mut(search_result.slot_index)
        {
            // Take the Option<V> out, replacing with None
            let value = slot.value.take();
            slot.clear();

            if let Some(metadata) = self.memory_pool.get_metadata_mut(search_result.slot_index) {
                metadata.mark_empty();
            }

            self.memory_pool.deallocate_slot(search_result.slot_index);
            return value; // Already Option<V> from slot.value.take()
        }

        None
    }
}
