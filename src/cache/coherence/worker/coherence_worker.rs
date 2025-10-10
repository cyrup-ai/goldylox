//! Background worker thread for coherence protocol operations
//!
//! This module implements the core worker that exclusively owns all coherence data
//! and processes requests via tokio channels.

use tokio::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use super::message_types::CoherenceRequest;
use crate::cache::coherence::communication::CoherenceError;
use crate::cache::coherence::data_structures::{
    CacheTier, CoherenceController, ProtocolConfiguration,
};
use crate::cache::traits::cache_entry::SerializationEnvelope;
use crate::cache::traits::{CacheKey, CacheValue};

/// Background worker that exclusively owns coherence controller
pub struct CoherenceWorker<
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
> {
    /// Exclusively owned coherence controller - no shared access
    controller: CoherenceController<K, V>,
    /// Request channel receiver
    request_rx: mpsc::UnboundedReceiver<CoherenceRequest<K, V>>,
    /// Worker shutdown signal
    shutdown_rx: mpsc::UnboundedReceiver<()>,
}

impl<
    K: CacheKey
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
    V: CacheValue
        + Default
        + PartialEq
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
> CoherenceWorker<K, V>
{
    /// Create new worker with owned controller
    pub fn new(
        config: ProtocolConfiguration,
        hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
        warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
        cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
        request_rx: mpsc::UnboundedReceiver<CoherenceRequest<K, V>>,
        shutdown_rx: mpsc::UnboundedReceiver<()>,
    ) -> Self {
        Self {
            controller: CoherenceController::new(config, hot_tier_coordinator, warm_tier_coordinator, cold_tier_coordinator),
            request_rx,
            shutdown_rx,
        }
    }

    /// Main worker processing loop - exclusively owns all coherence data
    pub async fn run(mut self) {
        let mut maintenance_interval = tokio::time::interval(Duration::from_millis(100));
        maintenance_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);

        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    // Graceful shutdown
                    break;
                }
                Some(request) = self.request_rx.recv() => {
                    self.process_request(request);
                }
                _ = maintenance_interval.tick() => {
                    // Periodic maintenance - process worker completions
                    self.controller.perform_maintenance().await;
                }
            }
        }
    }

    /// Process coherence request using exclusively owned controller
    fn process_request(&self, request: CoherenceRequest<K, V>) {
        match request {
            CoherenceRequest::Read {
                key,
                requesting_tier,
                response,
            } => {
                let result = self.controller.handle_read_request(&key, requesting_tier);
                let _ = response.send(result);
            }
            CoherenceRequest::Write {
                key,
                data,
                requesting_tier,
                response,
            } => {
                let result = self.controller.handle_write_request(&key, requesting_tier, data);
                let _ = response.send(result);
            }
            CoherenceRequest::GetStatistics { response } => {
                let stats = self.controller.get_statistics();
                let _ = response.send(stats);
            }
            CoherenceRequest::Serialize {
                key,
                value,
                target_tier,
                response,
            } => {
                let result = self.create_serialization_envelope_internal(key, value, target_tier);
                let _ = response.send(result.map(Box::new));
            }
            CoherenceRequest::RecordRead {
                key,
                tier,
                response,
            } => {
                // Record read access using existing coherence infrastructure
                let result = self.controller.handle_read_request(&key, tier).map(|_| ());
                let _ = response.send(result);
            }
            CoherenceRequest::RecordWrite {
                key,
                data,
                tier,
                response,
            } => {
                // Record write access using proper write request handler
                let result = self.controller.handle_write_request(&key, tier, data).map(|_| ());
                let _ = response.send(result);
            }
            CoherenceRequest::RecordPrefetch {
                key,
                tier,
                response,
            } => {
                // Record prefetch access - treated as read request for coherence purposes
                let result = self.controller.handle_read_request(&key, tier).map(|_| ());
                let _ = response.send(result);
            }
        }
    }

    /// Internal serialization envelope creation using owned controller
    fn create_serialization_envelope_internal(
        &self,
        key: K,
        value: V,
        target_tier: CacheTier,
    ) -> Result<SerializationEnvelope<K, V>, CoherenceError> {
        use crate::cache::coherence::data_structures::CoherenceKey;
        use crate::cache::coherence::write_propagation::types::WritePriority;
        use crate::cache::traits::cache_entry::{
            CacheEntry, SerializationEnvelope, TierSerializationContext,
        };
        use crate::cache::traits::types_and_enums::TierLocation;
        use std::time::{SystemTime, UNIX_EPOCH};

        // Create complete cache entry with tier-appropriate metadata
        let initial_tier = match target_tier {
            CacheTier::Hot => TierLocation::Hot,
            CacheTier::Warm => TierLocation::Warm,
            CacheTier::Cold => TierLocation::Cold,
        };
        let cache_entry = CacheEntry::new(key.clone(), value.clone(), initial_tier);

        let coherence_key = CoherenceKey::from_cache_key(&key);

        // Get checksum and compression from coherence metadata
        let (checksum, compression_used) = if let Some(cache_line_entry) =
            self.controller.cache_line_states.get(&coherence_key)
        {
            let cache_line = cache_line_entry.value();
            let checksum = cache_line
                .metadata
                .invalidation_seq
                .load(std::sync::atomic::Ordering::Acquire) as u64;

            let compression_used = match initial_tier {
                TierLocation::Hot => {
                    crate::cache::traits::supporting_types::CompressionAlgorithm::None
                }
                TierLocation::Warm => {
                    crate::cache::traits::supporting_types::CompressionAlgorithm::Lz4
                }
                TierLocation::Cold => {
                    crate::cache::traits::supporting_types::CompressionAlgorithm::Zstd
                }
            };

            (checksum, compression_used)
        } else {
            return Err(CoherenceError::CacheLineNotFound);
        };

        let serialized_at_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let tier_context = TierSerializationContext {
            target_tier: initial_tier,
            expected_frequency: cache_entry.access_tracker.frequency(),
            optimization_level: match initial_tier {
                TierLocation::Hot => 1,
                TierLocation::Warm => 5,
                TierLocation::Cold => 9,
            },
            include_access_history: initial_tier != TierLocation::Hot,
        };

        // Submit write-back request (spawn async operation in background)
        let write_propagation = Arc::clone(&self.controller.write_propagation);
        let key_clone = coherence_key.clone();
        let value_clone = value.clone();
        let dest_tier = self.controller.determine_target_tier(target_tier);
        tokio::runtime::Handle::current().spawn(async move {
            let _ = write_propagation.submit_writeback(
                key_clone,
                value_clone,
                target_tier,
                dest_tier,
                0,
                WritePriority::Normal,
            ).await;
        });

        Ok(SerializationEnvelope {
            entry: cache_entry,
            schema_version: 1,
            compression_used,
            checksum,
            serialized_at_ns,
            tier_context,
        })
    }
}
