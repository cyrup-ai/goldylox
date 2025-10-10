//! Worker manager for coherence system lifecycle
//!
//! Manages worker threads, channels, and provides external communication interface

use tokio::sync::mpsc;
use std::marker::PhantomData;

use super::coherence_worker::CoherenceWorker;
use super::message_types::CoherenceRequest;
use crate::cache::coherence::communication::CoherenceError;
use crate::cache::coherence::data_structures::ProtocolConfiguration;
use crate::cache::traits::{CacheKey, CacheValue};

/// Type alias for coherence worker channel sender to simplify complex type signatures
type CoherenceSenderChannel<K, V> = mpsc::UnboundedSender<CoherenceRequest<K, V>>;

/// External handle for sending requests to coherence worker
#[derive(Debug)]
pub struct CoherenceSender<
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
    request_tx: mpsc::UnboundedSender<CoherenceRequest<K, V>>,
    shutdown_tx: mpsc::UnboundedSender<()>,
}

/// Manager for coherence worker lifecycle
#[derive(Debug)]
pub struct CoherenceWorkerManager<
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
    config: ProtocolConfiguration,
    hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
    warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
    cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    _phantom: PhantomData<(K, V)>,
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
> CoherenceWorkerManager<K, V>
{
    /// Create new worker manager
    pub fn new(
        config: ProtocolConfiguration,
        hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
        warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
        cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    ) -> Result<Self, CoherenceError> {
        Ok(Self {
            config,
            hot_tier_coordinator,
            warm_tier_coordinator,
            cold_tier_coordinator,
            worker_handle: None,
            _phantom: PhantomData,
        })
    }

    /// Start worker thread and return communication handle
    pub fn start_worker(&mut self) -> Result<CoherenceSender<K, V>, CoherenceError> {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();

        let worker = CoherenceWorker::new(
            self.config.clone(),
            self.hot_tier_coordinator.clone(),
            self.warm_tier_coordinator.clone(),
            self.cold_tier_coordinator.clone(),
            request_rx,
            shutdown_rx,
        );

        let handle = tokio::runtime::Handle::current().spawn(async move {
            worker.run().await;
        });

        self.worker_handle = Some(handle);

        Ok(CoherenceSender {
            request_tx,
            shutdown_tx,
        })
    }

    /// Shutdown worker gracefully
    pub fn shutdown(self) -> Result<(), CoherenceError> {
        if let Some(_handle) = self.worker_handle {
            // Task will be dropped, which cancels it
        }
        Ok(())
    }
}

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
> CoherenceSender<K, V>
{
    /// Send request to worker (caller provides oneshot channel for response)
    pub fn send_request(&self, request: CoherenceRequest<K, V>) -> Result<(), CoherenceError> {
        self.request_tx
            .send(request)
            .map_err(|_| CoherenceError::ChannelClosed)
    }

    /// Initiate graceful shutdown of the worker thread
    pub fn shutdown(&self) -> Result<(), CoherenceError> {
        self.shutdown_tx
            .send(())
            .map_err(|_| CoherenceError::ChannelClosed)
    }

    /// Get sender for direct channel access
    pub fn get_request_sender(&self) -> mpsc::UnboundedSender<CoherenceRequest<K, V>> {
        self.request_tx.clone()
    }
}

/// Get worker channels for coherence communication (per-instance)
pub fn get_worker_channels<
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
>(
    channel_map: &std::sync::Arc<std::sync::RwLock<
        std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>
    >>
) -> Option<mpsc::UnboundedSender<CoherenceRequest<K, V>>> {
    if let Ok(map) = channel_map.read() {
        let type_id = std::any::TypeId::of::<(K, V)>();
        if let Some(channel_any) = map.get(&type_id) {
            // Safely downcast the boxed Any to the correct channel sender type
            if let Some(sender) =
                channel_any.downcast_ref::<mpsc::UnboundedSender<CoherenceRequest<K, V>>>()
            {
                return Some(sender.clone());
            }
        }
    }

    None
}

/// Register worker channels for a specific key-value type combination (per-instance)
pub fn register_worker_channels<
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
>(
    channel_map: &std::sync::Arc<std::sync::RwLock<
        std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>
    >>,
    sender: mpsc::UnboundedSender<CoherenceRequest<K, V>>,
) {
    if let Ok(mut map) = channel_map.write() {
        let type_id = std::any::TypeId::of::<(K, V)>();
        map.insert(type_id, Box::new(sender));
    }
}
