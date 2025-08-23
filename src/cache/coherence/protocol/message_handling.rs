//! Message handling for coherence protocol
//!
//! This module implements processing of incoming coherence messages
//! and coordination of background operations.

use std::time::Instant;

use crate::cache::coherence::communication::{CoherenceError, CoherenceMessage};
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey};
use crate::cache::coherence::invalidation::InvalidationPriority;
use super::types::CoherenceController;
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey, V: CacheValue> CoherenceController<K, V> {
    /// Process pending coherence operations
    pub fn process_pending_operations(&self) {
        // Process pending invalidations
        let invalidation_messages = self.invalidation_manager.process_pending();
        for message in invalidation_messages {
            if let Err(_) = self.communication_hub.broadcast(message) {
                self.coherence_stats.record_failure();
            }
        }

        // Process pending write-backs
        let writeback_tasks = self.write_propagation.process_writebacks();
        for _task in writeback_tasks {
            // In a real implementation, these would be sent to background workers
            // For now, we'll just record the processing
            self.coherence_stats.record_writeback();
        }
    }

    /// Handle incoming coherence message
    pub fn handle_coherence_message(
        &self,
        message: CoherenceMessage<K, V>,
    ) -> Result<(), CoherenceError> {
        match message {
            CoherenceMessage::RequestExclusive {
                key,
                requester_tier,
                ..
            } => self.handle_exclusive_request(key, requester_tier),
            CoherenceMessage::RequestShared {
                key,
                requester_tier,
                ..
            } => self.handle_shared_request(key, requester_tier),
            CoherenceMessage::Invalidate {
                key,
                target_tier,
                reason,
                ..
            } => self.handle_invalidation_message(key, target_tier, reason),
            CoherenceMessage::WriteBack {
                key, source_tier, ..
            } => self.handle_writeback_message(key, source_tier),
            _ => {
                // Handle other message types as needed
                Ok(())
            }
        }
    }

    fn handle_exclusive_request(
        &self,
        key: CoherenceKey<K>,
        requester_tier: CacheTier,
    ) -> Result<(), CoherenceError> {
        // Grant exclusive access and invalidate other copies
        let grant_message = CoherenceMessage::GrantExclusive {
            key: key.clone(),
            target_tier: requester_tier,
            version: 0,
            timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
        };

        self.communication_hub
            .send_to_tier(requester_tier, grant_message)?;

        // Invalidate other tiers
        self.invalidation_manager.submit_invalidation(
            key,
            self.get_other_tier(requester_tier),
            crate::cache::coherence::InvalidationReason::WriteConflict,
            InvalidationPriority::High,
        );

        Ok(())
    }

    fn handle_shared_request(
        &self,
        key: CoherenceKey<K>,
        requester_tier: CacheTier,
    ) -> Result<(), CoherenceError> {
        // Grant shared access
        let grant_message = CoherenceMessage::GrantShared {
            key,
            target_tier: requester_tier,
            version: 0,
            timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
        };

        self.communication_hub
            .send_to_tier(requester_tier, grant_message)?;
        Ok(())
    }

    fn handle_invalidation_message(
        &self,
        key: CoherenceKey<K>,
        _target_tier: CacheTier,
        _reason: crate::cache::coherence::InvalidationReason,
    ) -> Result<(), CoherenceError> {
        // Mark invalidation as completed
        self.invalidation_manager.mark_completed(&key, true);
        self.coherence_stats.record_invalidation_received();
        Ok(())
    }

    fn handle_writeback_message(
        &self,
        _key: CoherenceKey<K>,
        _source_tier: CacheTier,
    ) -> Result<(), CoherenceError> {
        // Handle write-back completion
        self.coherence_stats.record_writeback();
        Ok(())
    }
}
