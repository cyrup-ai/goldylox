//! Read operation handling for coherence protocol
//!
//! This module implements read request processing including cache hits,
//! misses, and state transitions for shared access.

use std::time::Instant;

use super::super::communication::{CoherenceError, CoherenceMessage, ReadResponse};
use super::super::data_structures::{CacheTier, CoherenceKey, MesiState};
use super::super::state_management::{StateTransitionRequest, TransitionReason};
use super::types::CoherenceController;
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey, V: CacheValue> CoherenceController<K, V> {
    /// Handle read request from a cache tier
    pub fn handle_read_request(
        &self,
        key: &K,
        requesting_tier: CacheTier,
    ) -> Result<ReadResponse, CoherenceError> {
        let start_time = Instant::now();
        let coherence_key = CoherenceKey::from_cache_key(key);

        // Get or create cache line state
        let cache_line = self.cache_line_states.get_or_insert(
            coherence_key.clone(),
            super::super::data_structures::CacheLineState::new(),
        );

        let current_state = cache_line.value().get_mesi_state();
        cache_line.value().increment_access_count();

        let response = match current_state {
            MesiState::Invalid => {
                // Cache miss - need to fetch from lower tier
                self.handle_cache_miss(&coherence_key, requesting_tier, false)?
            }
            MesiState::Shared | MesiState::Exclusive | MesiState::Modified => {
                // Cache hit - data is available
                ReadResponse::Hit
            }
        };

        // Record statistics
        let latency_ns = start_time.elapsed().as_nanos() as u64;
        self.coherence_stats.record_success(latency_ns);

        Ok(response)
    }

    /// Handle cache miss (read or write)
    pub fn handle_cache_miss(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        is_write: bool,
    ) -> Result<ReadResponse, CoherenceError> {
        // Send request to communication hub
        let message = if is_write {
            CoherenceMessage::RequestExclusive {
                key: key.clone(),
                requester_tier: requesting_tier,
                version: 0,
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
            }
        } else {
            CoherenceMessage::RequestShared {
                key: key.clone(),
                requester_tier: requesting_tier,
                version: 0,
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
            }
        };

        // Broadcast request to other tiers
        self.communication_hub.broadcast(message)?;

        // Update cache line state based on response
        if let Some(cache_line_entry) = self.cache_line_states.get(key) {
            let cache_line = cache_line_entry.value();
            let new_state = if is_write {
                MesiState::Exclusive
            } else {
                MesiState::Shared
            };

            let transition_request = StateTransitionRequest {
                from_state: cache_line.get_mesi_state(),
                to_state: new_state,
                tier: requesting_tier,
                version: cache_line.get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: if is_write {
                    TransitionReason::Write
                } else {
                    TransitionReason::Read
                },
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.set_mesi_state(new_state);
            cache_line.increment_version();
        }

        Ok(ReadResponse::SharedGranted)
    }
}
