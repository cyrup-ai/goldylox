//! Write operation handling for coherence protocol
//!
//! This module implements write request processing including write misses,
//! shared writes, and exclusive writes with proper state transitions.

use std::time::Instant;

use crate::cache::coherence::communication::{CoherenceError, WriteResponse};
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey, MesiState};
use crate::cache::coherence::invalidation::InvalidationPriority;
use crate::cache::coherence::state_management::{StateTransitionRequest, TransitionReason};
use crate::cache::coherence::write_propagation::WritePriority;
use super::types::CoherenceController;
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey, V: CacheValue> CoherenceController<K, V> {
    /// Handle write request from a cache tier
    pub fn handle_write_request(
        &self,
        key: &K,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<WriteResponse, CoherenceError> {
        let start_time = Instant::now();
        let coherence_key = CoherenceKey::from_cache_key(key);

        // Get or create cache line state
        let cache_line = self.cache_line_states.get_or_insert(
            coherence_key.clone(),
            crate::cache::coherence::CacheLineState::new(),
        );

        let current_state = cache_line.value().get_mesi_state();
        cache_line.value().increment_access_count();

        let response = match current_state {
            MesiState::Invalid => {
                // Write miss - need exclusive access
                self.handle_write_miss(&coherence_key, requesting_tier, data)?
            }
            MesiState::Shared => {
                // Need to invalidate other sharers and get exclusive access
                self.handle_shared_write(&coherence_key, requesting_tier, data)?
            }
            MesiState::Exclusive | MesiState::Modified => {
                // Can write directly
                self.handle_exclusive_write(&coherence_key, requesting_tier, data)?
            }
        };

        // Record statistics
        let latency_ns = start_time.elapsed().as_nanos() as u64;
        self.coherence_stats.record_success(latency_ns);

        Ok(response)
    }

    /// Handle write miss
    pub fn handle_write_miss(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<WriteResponse, CoherenceError> {
        self.handle_cache_miss(key, requesting_tier, true)?;

        // Submit write-back request
        self.write_propagation.submit_writeback(
            key.clone(),
            data,
            requesting_tier,
            self.determine_target_tier(requesting_tier),
            0,
            WritePriority::Normal,
        );

        Ok(WriteResponse::Success)
    }

    /// Handle write to shared cache line
    pub fn handle_shared_write(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<WriteResponse, CoherenceError> {
        // Send invalidation to other tiers
        self.invalidation_manager.submit_invalidation(
            key.clone(),
            self.get_other_tier(requesting_tier),
            crate::cache::coherence::InvalidationReason::WriteConflict,
            InvalidationPriority::High,
        );

        // Transition to exclusive state
        if let Some(cache_line_entry) = self.cache_line_states.get(key) {
            let cache_line = cache_line_entry.value();
            let transition_request = StateTransitionRequest {
                from_state: MesiState::Shared,
                to_state: MesiState::Exclusive,
                tier: requesting_tier,
                version: cache_line.get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: TransitionReason::Write,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.set_mesi_state(MesiState::Exclusive);
            cache_line.increment_version();
        }

        // Submit write-back request
        self.write_propagation.submit_writeback(
            key.clone(),
            data,
            requesting_tier,
            self.determine_target_tier(requesting_tier),
            0,
            WritePriority::High,
        );

        Ok(WriteResponse::Success)
    }

    /// Handle write to exclusive cache line
    pub fn handle_exclusive_write(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<WriteResponse, CoherenceError> {
        // Transition to modified state
        if let Some(cache_line_entry) = self.cache_line_states.get(key) {
            let cache_line = cache_line_entry.value();
            let current_state = cache_line.get_mesi_state();
            let transition_request = StateTransitionRequest {
                from_state: current_state,
                to_state: MesiState::Modified,
                tier: requesting_tier,
                version: cache_line.get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: TransitionReason::Write,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.set_mesi_state(MesiState::Modified);
            cache_line.increment_version();
        }

        // Submit write-back request with lower priority (already exclusive)
        self.write_propagation.submit_writeback(
            key.clone(),
            data,
            requesting_tier,
            self.determine_target_tier(requesting_tier),
            0,
            WritePriority::Normal,
        );

        Ok(WriteResponse::Success)
    }
}
