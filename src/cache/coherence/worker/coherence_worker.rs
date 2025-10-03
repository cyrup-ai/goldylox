//! Background worker thread for coherence protocol operations
//!
//! This module implements the core worker that exclusively owns all coherence data
//! and processes requests via crossbeam channels.

use crossbeam_channel::{Receiver, RecvError, Sender};

use super::message_types::{CoherenceRequest, CoherenceResponse};
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
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
> {
    /// Exclusively owned coherence controller - no shared access
    controller: CoherenceController<K, V>,
    /// Request channel receiver
    request_rx: Receiver<CoherenceRequest<K, V>>,
    /// Response channel sender
    response_tx: Sender<CoherenceResponse<K, V>>,
    /// Worker shutdown signal
    shutdown_rx: Receiver<()>,
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
        hot_tier_coordinator: std::sync::Arc<crate::cache::tier::hot::thread_local::HotTierCoordinator>,
        warm_tier_coordinator: std::sync::Arc<crate::cache::tier::warm::global_api::WarmTierCoordinator>,
        cold_tier_coordinator: std::sync::Arc<crate::cache::tier::cold::ColdTierCoordinator>,
        request_rx: Receiver<CoherenceRequest<K, V>>,
        response_tx: Sender<CoherenceResponse<K, V>>,
        shutdown_rx: Receiver<()>,
    ) -> Self {
        Self {
            controller: CoherenceController::new(config, hot_tier_coordinator, warm_tier_coordinator, cold_tier_coordinator),
            request_rx,
            response_tx,
            shutdown_rx,
        }
    }

    /// Main worker processing loop - exclusively owns all coherence data
    pub fn run(self) {
        use std::time::Duration;

        loop {
            crossbeam_channel::select! {
                recv(self.shutdown_rx) -> _ => {
                    // Graceful shutdown
                    break;
                }
                recv(self.request_rx) -> request => {
                    match request {
                        Ok(req) => {
                            let response = self.process_request(req);
                            if self.response_tx.send(response).is_err() {
                                // Response channel closed - shutdown
                                break;
                            }
                        }
                        Err(RecvError) => {
                            // Request channel closed - shutdown
                            break;                        }
                    }
                }
                default(Duration::from_millis(100)) => {
                    // Periodic maintenance - process worker completions
                    self.controller.perform_maintenance();
                }
            }
        }
    }

    /// Process coherence request using exclusively owned controller
    fn process_request(&self, request: CoherenceRequest<K, V>) -> CoherenceResponse<K, V> {
        match request {
            CoherenceRequest::Read {
                key,
                requesting_tier,
                request_id,
            } => match self.controller.handle_read_request(&key, requesting_tier) {
                Ok(read_response) => CoherenceResponse::ReadSuccess {
                    request_id,
                    response: read_response,
                },
                Err(error) => CoherenceResponse::Error { request_id, error },
            },
            CoherenceRequest::Write {
                key,
                data,
                requesting_tier,
                request_id,
            } => {
                match self
                    .controller
                    .handle_write_request(&key, requesting_tier, data)
                {
                    Ok(write_response) => CoherenceResponse::WriteSuccess {
                        request_id,
                        response: write_response,
                    },
                    Err(error) => CoherenceResponse::Error { request_id, error },
                }
            }
            CoherenceRequest::GetStatistics { request_id } => {
                let stats = self.controller.get_statistics();
                CoherenceResponse::Statistics {
                    request_id,
                    statistics: stats,
                }
            }
            CoherenceRequest::Serialize {
                key,
                value,
                target_tier,
                request_id,
            } => match self.create_serialization_envelope_internal(key, value, target_tier) {
                Ok(envelope) => CoherenceResponse::SerializeSuccess {
                    request_id,
                    envelope: Box::new(envelope),
                },
                Err(error) => CoherenceResponse::Error { request_id, error },
            },
            CoherenceRequest::RecordRead {
                key,
                tier,
                request_id,
            } => {
                // Record read access using existing coherence infrastructure
                match self.controller.handle_read_request(&key, tier) {
                    Ok(_) => CoherenceResponse::AccessRecorded { request_id },
                    Err(error) => CoherenceResponse::Error { request_id, error },
                }
            }
            CoherenceRequest::RecordWrite {
                key,
                data,
                tier,
                request_id,
            } => {
                // Record write access using proper write request handler
                match self.controller.handle_write_request(&key, tier, data) {
                    Ok(_write_response) => CoherenceResponse::AccessRecorded { request_id },
                    Err(error) => CoherenceResponse::Error { request_id, error },
                }
            }
            CoherenceRequest::RecordPrefetch {
                key,
                tier,
                request_id,
            } => {
                // Record prefetch access - treated as read request for coherence purposes
                match self.controller.handle_read_request(&key, tier) {
                    Ok(_) => CoherenceResponse::AccessRecorded { request_id },
                    Err(error) => CoherenceResponse::Error { request_id, error },
                }
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

        self.controller.write_propagation.submit_writeback(
            coherence_key.clone(),
            value.clone(),
            target_tier,
            self.controller.determine_target_tier(target_tier),
            0,
            WritePriority::Normal,
        );

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
