//! Exclusive access management for coherence protocol
//!
//! This module handles exclusive access requests and cache line invalidation
//! operations to maintain coherence across cache tiers.

// Internal coherence architecture - components may not be used in minimal API

use std::time::Instant;

use crate::cache::coherence::communication::{CoherenceError, ExclusiveResponse};
use crate::cache::coherence::data_structures::CoherenceController;
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey, MesiState};
use crate::cache::coherence::invalidation::types::InvalidationPriority;
use crate::cache::coherence::state_management::{StateTransitionRequest, TransitionReason};
use crate::cache::traits::{CacheKey, CacheValue};

impl<
    K: CacheKey
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
    V: CacheValue
        + Default
        + PartialEq
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
> CoherenceController<K, V>
{
    /// Request exclusive access to a cache line
    #[allow(dead_code)] // MESI coherence - used in protocol exclusive access management
    pub fn request_exclusive_access(
        &self,
        key: &K,
        requesting_tier: CacheTier,
    ) -> Result<ExclusiveResponse, CoherenceError> {
        let coherence_key = CoherenceKey::from_cache_key(key);

        // Check current state
        if let Some(cache_line) = self.cache_line_states.get(&coherence_key) {
            let current_state = cache_line.value().get_mesi_state();

            match current_state {
                MesiState::Invalid => {
                    // Need to fetch from lower tier
                    self.handle_cache_miss(&coherence_key, requesting_tier, true)?;
                    Ok(ExclusiveResponse::Granted)
                }
                MesiState::Shared => {
                    // Need to invalidate other sharers
                    self.invalidation_manager.submit_invalidation(
                        coherence_key,
                        self.get_other_tier(requesting_tier),
                        crate::cache::coherence::InvalidationReason::WriteConflict,
                        InvalidationPriority::High,
                    );
                    Ok(ExclusiveResponse::Granted)
                }
                MesiState::Exclusive | MesiState::Modified => {
                    // Already have exclusive access
                    Ok(ExclusiveResponse::Granted)
                }
            }
        } else {
            // Cache line doesn't exist, create it
            let cache_line = crate::cache::coherence::CacheLineState::new();
            cache_line.set_mesi_state(MesiState::Exclusive);
            self.cache_line_states.insert(coherence_key, cache_line);
            Ok(ExclusiveResponse::Granted)
        }
    }

    /// Invalidate cache line in specific tier
    #[allow(dead_code)] // MESI coherence - used in protocol cache line invalidation operations
    pub fn invalidate_cache_line(
        &self,
        key: &K,
        target_tier: CacheTier,
        reason: crate::cache::coherence::InvalidationReason,
    ) -> Result<(), CoherenceError> {
        let coherence_key = CoherenceKey::from_cache_key(key);

        // Submit invalidation request
        self.invalidation_manager.submit_invalidation(
            coherence_key.clone(),
            target_tier,
            reason,
            InvalidationPriority::Normal,
        );

        // Update local cache line state if it exists
        if let Some(cache_line) = self.cache_line_states.get(&coherence_key) {
            let transition_request = StateTransitionRequest {
                from_state: cache_line.value().get_mesi_state(),
                to_state: MesiState::Invalid,
                tier: target_tier,
                version: cache_line.value().get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: TransitionReason::Invalidation,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.value().set_mesi_state(MesiState::Invalid);
            cache_line.value().increment_version();
        }

        self.coherence_stats.record_invalidation_sent();
        Ok(())
    }
}
