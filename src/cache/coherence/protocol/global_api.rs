//! Global API functions for coherence protocol
//!
//! This module provides the public interface for external use of the
//! coherence protocol with simplified function signatures.

// Internal protocol architecture - components may not be used in minimal API

use crate::cache::coherence::CacheTier;
use crate::cache::coherence::communication::{CoherenceError, ReadResponse, WriteResponse};
use crate::cache::coherence::data_structures::ProtocolConfiguration;
use crate::cache::coherence::worker::message_types::CoherenceRequest;
use crate::cache::coherence::worker::worker_manager::{CoherenceSender, CoherenceWorkerManager};
use crate::cache::traits::cache_entry::SerializationEnvelope;
use crate::cache::traits::{CacheKey, CacheValue};

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Coherence system handle that manages worker lifecycle
#[derive(Debug)]
pub struct CoherenceSystem<
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
> {
    sender: CoherenceSender<K, V>,
    #[allow(dead_code)] // MESI coherence - worker manager used for lifecycle management
    manager: CoherenceWorkerManager<K, V>,
    coherence_stats: crate::cache::coherence::statistics::core_statistics::CoherenceStatistics,
    /// Per-instance request ID counter for coherence protocol
    request_id_counter: AtomicU64,
    
    /// Per-instance coherence worker channel registry
    /// Type-erased storage for coherence protocol worker channels per K,V type
    worker_channels: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<
        std::any::TypeId,
        Box<dyn std::any::Any + Send + Sync>
    >>>,
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
> CoherenceSystem<K, V>
{
    /// Initialize coherence system with worker-owned architecture
    pub fn new(
        hot_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
        warm_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
        cold_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    ) -> Result<Self, CoherenceError> {
        // Create per-instance channel map
        let worker_channels = std::sync::Arc::new(
            std::sync::RwLock::new(std::collections::HashMap::new())
        );
        
        // Use coordinators passed as parameters
        let mut manager = CoherenceWorkerManager::<K, V>::new(
            ProtocolConfiguration::default(),
            hot_coordinator,
            warm_coordinator,
            cold_coordinator,
        )?;
        let sender = manager.start_worker()?;
        let coherence_stats = crate::cache::coherence::statistics::core_statistics::CoherenceStatistics::new();
        Ok(Self { 
            sender, 
            manager, 
            coherence_stats,
            request_id_counter: AtomicU64::new(1),
            worker_channels,
        })
    }

    /// Get reference to worker channels for this instance
    pub fn worker_channels(&self) -> &std::sync::Arc<std::sync::RwLock<
        std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>
    >> {
        &self.worker_channels
    }

    /// Get coherence statistics reference
    pub fn coherence_stats(&self) -> &crate::cache::coherence::statistics::core_statistics::CoherenceStatistics {
        &self.coherence_stats
    }

    /// Get reference to request ID counter for coherence requests
    pub fn request_id_counter(&self) -> &AtomicU64 {
        &self.request_id_counter
    }

    /// Get sender for communication with worker
    pub fn sender(&self) -> &CoherenceSender<K, V> {
        &self.sender
    }

    /// Shutdown the coherence system gracefully
    #[allow(dead_code)] // MESI coherence - graceful shutdown used in system lifecycle
    pub fn shutdown(self) -> Result<(), CoherenceError> {
        self.sender.shutdown()?;
        self.manager.shutdown()
    }
}

/// Perform coherent read operation using channel-based worker communication
///
/// This function sends read requests to the background worker that exclusively owns
/// all coherence data. No shared controller access - pure message passing.
#[allow(dead_code)] // MESI coherence - used in protocol coherent read operations  
pub async fn coherent_read<K, V>(
    system: &CoherenceSystem<K, V>,
    key: K,
    requesting_tier: CacheTier,
) -> Result<ReadResponse, CoherenceError>
where
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
{
    use tokio::sync::oneshot;
    
    let sender = system.sender();
    let (response_tx, response_rx) = oneshot::channel();
    let request = CoherenceRequest::Read {
        key,
        requesting_tier,
        response: response_tx,
    };

    // Send request to worker via channel
    sender.send_request(request)?;

    // Wait for response with timeout
    tokio::time::timeout(Duration::from_secs(5), response_rx)
        .await
        .map_err(|_| CoherenceError::TimeoutExpired)?
        .map_err(|_| CoherenceError::ChannelClosed)?
}

/// Perform coherent write operation using channel-based worker communication
///
/// This function sends write requests to the background worker that exclusively owns
/// all coherence data. Write propagation handled by worker thread.
#[allow(dead_code)] // MESI coherence - used in protocol coherent write operations  
pub async fn coherent_write<K, V>(
    system: &CoherenceSystem<K, V>,
    key: K,
    data: V,
    requesting_tier: CacheTier,
) -> Result<WriteResponse, CoherenceError>
where
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
{
    use tokio::sync::oneshot;
    
    let sender = system.sender();
    let (response_tx, response_rx) = oneshot::channel();
    let request = CoherenceRequest::Write {
        key,
        data,
        requesting_tier,
        response: response_tx,
    };

    // Send request to worker via channel
    sender.send_request(request)?;

    // Wait for response with timeout
    tokio::time::timeout(Duration::from_secs(5), response_rx)
        .await
        .map_err(|_| CoherenceError::TimeoutExpired)?
        .map_err(|_| CoherenceError::ChannelClosed)?
}

/// Serialize cache value with comprehensive envelope metadata using coherence integration
///
/// Creates a SerializationEnvelope with complete metadata for tier-aware serialization following
/// the production pattern. Uses channel-based worker communication for exclusive coherence access.
#[allow(dead_code)] // MESI coherence - used in protocol tier-aware serialization operations  
pub async fn serialize_tier_value_with_envelope<K, V>(
    system: &CoherenceSystem<K, V>,
    key: K,
    value: V,
    target_tier: CacheTier,
) -> Result<SerializationEnvelope<K, V>, CoherenceError>
where
    K: CacheKey
        + Clone
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
    V: CacheValue
        + Clone
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
{
    use tokio::sync::oneshot;
    
    let sender = system.sender();
    let (response_tx, response_rx) = oneshot::channel();
    let request = CoherenceRequest::Serialize {
        key,
        value,
        target_tier,
        response: response_tx,
    };

    // Send request to worker via channel
    sender.send_request(request)?;

    // Wait for response with timeout
    let envelope = tokio::time::timeout(Duration::from_secs(5), response_rx)
        .await
        .map_err(|_| CoherenceError::TimeoutExpired)?
        .map_err(|_| CoherenceError::ChannelClosed)??;
    Ok(*envelope)
}
