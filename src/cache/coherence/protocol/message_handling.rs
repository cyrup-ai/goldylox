//! Message handling for coherence protocol
//!
//! This module implements processing of incoming coherence messages
//! and coordination of background operations.

// Internal protocol architecture - components may not be used in minimal API

use std::time::Instant;

use crate::cache::coherence::communication::{CoherenceError, CoherenceMessage};
use crate::cache::coherence::data_structures::CoherenceController;
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey};
use crate::cache::coherence::invalidation::types::InvalidationPriority;
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
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
> CoherenceController<K, V>
{
    /// Process pending coherence operations
    #[allow(dead_code)] // MESI coherence - used in protocol background operation processing
    pub fn process_pending_operations(&self) {
        // Process pending invalidations
        let invalidation_messages = self.invalidation_manager.process_pending();
        for message in invalidation_messages {
            if self.communication_hub.broadcast(message).is_err() {
                self.coherence_stats.record_failure();
            }
        }

        // Process pending write-backs
        let writeback_tasks = self.write_propagation.process_writebacks();
        for task in writeback_tasks {
            // Send task to background worker via existing channel
            if self
                .write_propagation
                .worker_channels
                .task_tx
                .try_send(task)
                .is_err()
            {
                // Worker queue is full, record failure and continue
                self.coherence_stats.record_failure();
            } else {
                // Successfully submitted to worker
                self.coherence_stats.record_writeback();
            }
        }
    }

    /// Handle incoming coherence message
    #[allow(dead_code)] // MESI coherence - used in protocol message processing and handling
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

    #[allow(dead_code)] // MESI coherence - used in protocol exclusive access request handling
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

    #[allow(dead_code)] // MESI coherence - used in protocol shared access request handling
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

    #[allow(dead_code)] // MESI coherence - used in protocol invalidation message handling
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

    #[allow(dead_code)] // MESI coherence - used in protocol writeback message handling
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
