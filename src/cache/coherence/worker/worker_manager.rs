//! Worker manager for coherence system lifecycle
//!
//! Manages worker threads, channels, and provides external communication interface

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use dashmap::DashMap;
use std::marker::PhantomData;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::coherence_worker::CoherenceWorker;
use super::message_types::{CoherenceRequest, CoherenceResponse};
use crate::cache::coherence::communication::CoherenceError;
use crate::cache::coherence::data_structures::ProtocolConfiguration;
use crate::cache::traits::{CacheKey, CacheValue};

/// Type alias for coherence worker channel pair to simplify complex type signatures
type CoherenceChannelPair<K, V> = (
    Sender<CoherenceRequest<K, V>>,
    Receiver<CoherenceResponse<K, V>>,
);

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
    request_tx: Sender<CoherenceRequest<K, V>>,
    response_rx: Receiver<CoherenceResponse<K, V>>,
    shutdown_tx: Sender<()>,
    response_buffer: DashMap<u64, CoherenceResponse<K, V>>,
    buffer_limit: usize,
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
    worker_handle: Option<JoinHandle<()>>,
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
    pub fn new(config: ProtocolConfiguration) -> Result<Self, CoherenceError> {
        Ok(Self {
            config,
            worker_handle: None,
            _phantom: PhantomData,
        })
    }

    /// Start worker thread and return communication handle
    pub fn start_worker(&mut self) -> Result<CoherenceSender<K, V>, CoherenceError> {
        let (request_tx, request_rx) = unbounded();
        let (response_tx, response_rx) = unbounded();
        let (shutdown_tx, shutdown_rx) = unbounded();

        let worker =
            CoherenceWorker::new(self.config.clone(), request_rx, response_tx, shutdown_rx);

        let handle = thread::spawn(move || {
            worker.run();
        });

        self.worker_handle = Some(handle);

        Ok(CoherenceSender {
            request_tx,
            response_rx,
            shutdown_tx,
            response_buffer: DashMap::new(),
            buffer_limit: 1000, // Prevent unbounded growth
        })
    }

    /// Shutdown worker gracefully
    pub fn shutdown(self) -> Result<(), CoherenceError> {
        if let Some(handle) = self.worker_handle {
            handle
                .join()
                .map_err(|_| CoherenceError::CommunicationFailure)?;
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
    /// Send request to worker
    pub fn send_request(&self, request: CoherenceRequest<K, V>) -> Result<(), CoherenceError> {
        self.request_tx
            .send(request)
            .map_err(|_| CoherenceError::ChannelClosed)
    }

    /// Receive response with timeout
    pub fn receive_response(
        &self,
        request_id: u64,
        timeout: Duration,
    ) -> Result<CoherenceResponse<K, V>, CoherenceError> {
        // Check buffer first for previously received responses
        if let Some(response) = self.response_buffer.remove(&request_id) {
            return Ok(response.1); // DashMap::remove returns (key, value) tuple
        }

        // Receive responses until we find the target or timeout
        let start_time = std::time::Instant::now();
        loop {
            let elapsed = start_time.elapsed();
            if elapsed >= timeout {
                return Err(CoherenceError::TimeoutExpired);
            }

            let remaining_timeout = timeout - elapsed;
            match self.response_rx.recv_timeout(remaining_timeout) {
                Ok(response) => {
                    let response_id = self.extract_request_id(&response);
                    if response_id == request_id {
                        return Ok(response);
                    } else {
                        // Buffer out-of-order response for later retrieval
                        // Prevent unbounded growth by limiting buffer size
                        if self.response_buffer.len() < self.buffer_limit {
                            self.response_buffer.insert(response_id, response);
                        } else {
                            // Buffer full - remove oldest entry to make room
                            if let Some(entry) = self.response_buffer.iter().next() {
                                let oldest_id = *entry.key();
                                self.response_buffer.remove(&oldest_id);
                            }
                            self.response_buffer.insert(response_id, response);
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => return Err(CoherenceError::TimeoutExpired),
                Err(RecvTimeoutError::Disconnected) => return Err(CoherenceError::ChannelClosed),
            }
        }
    }

    fn extract_request_id(&self, response: &CoherenceResponse<K, V>) -> u64 {
        match response {
            CoherenceResponse::ReadSuccess { request_id, .. }
            | CoherenceResponse::WriteSuccess { request_id, .. }
            | CoherenceResponse::Statistics { request_id, .. }
            | CoherenceResponse::SerializeSuccess { request_id, .. }
            | CoherenceResponse::AccessRecorded { request_id, .. }
            | CoherenceResponse::Error { request_id, .. } => *request_id,
        }
    }

    /// Initiate graceful shutdown of the worker thread
    pub fn shutdown(&self) -> Result<(), CoherenceError> {
        self.shutdown_tx
            .send(())
            .map_err(|_| CoherenceError::ChannelClosed)
    }

    /// Clear response buffer to prevent memory leaks
    #[allow(dead_code)] // MESI coherence - used in worker maintenance and memory management
    pub fn clear_buffer(&self) -> Result<usize, CoherenceError> {
        let cleared_count = self.response_buffer.len();
        self.response_buffer.clear();
        Ok(cleared_count)
    }

    /// Get sender for direct channel access
    pub fn get_request_sender(&self) -> Sender<CoherenceRequest<K, V>> {
        self.request_tx.clone()
    }

    /// Get receiver for direct channel access
    pub fn get_response_receiver(&self) -> Receiver<CoherenceResponse<K, V>> {
        self.response_rx.clone()
    }
}

use std::any::TypeId;
use std::collections::HashMap;
/// Global worker channel registry
use std::sync::OnceLock;

static WORKER_CHANNELS: OnceLock<
    std::sync::RwLock<HashMap<TypeId, Box<dyn std::any::Any + Send + Sync>>>,
> = OnceLock::new();

/// Get worker channels for coherence communication
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
>() -> Option<CoherenceChannelPair<K, V>> {
    let channels = WORKER_CHANNELS.get_or_init(|| std::sync::RwLock::new(HashMap::new()));

    if let Ok(channel_map) = channels.read() {
        let type_id = TypeId::of::<(K, V)>();
        if let Some(channel_any) = channel_map.get(&type_id) {
            // Safely downcast the boxed Any to the correct channel pair type
            if let Some((sender, receiver)) =
                channel_any.downcast_ref::<CoherenceChannelPair<K, V>>()
            {
                return Some((sender.clone(), receiver.clone()));
            }
        }
    }

    None
}

/// Register worker channels for a specific key-value type combination  
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
    channels: CoherenceChannelPair<K, V>,
) {
    let registry = WORKER_CHANNELS.get_or_init(|| std::sync::RwLock::new(HashMap::new()));

    if let Ok(mut channel_map) = registry.write() {
        let type_id = TypeId::of::<(K, V)>();
        channel_map.insert(type_id, Box::new(channels));
    }
}
