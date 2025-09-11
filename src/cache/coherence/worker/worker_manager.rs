//! Worker manager for coherence system lifecycle
//!
//! Manages worker threads, channels, and provides external communication interface

use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crossbeam_channel::{unbounded, Sender, Receiver, RecvTimeoutError};

use crate::cache::coherence::data_structures::ProtocolConfiguration;
use crate::cache::coherence::communication::CoherenceError;
use crate::cache::traits::{CacheKey, CacheValue};
use super::coherence_worker::CoherenceWorker;
use super::message_types::{CoherenceRequest, CoherenceResponse};

/// External handle for sending requests to coherence worker
#[derive(Debug)]
pub struct CoherenceSender<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned, V: CacheValue + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned> {
    request_tx: Sender<CoherenceRequest<K, V>>,
    response_rx: Receiver<CoherenceResponse<K, V>>,
    shutdown_tx: Sender<()>,
    response_buffer: Arc<Mutex<HashMap<u64, CoherenceResponse<K, V>>>>,
    buffer_limit: usize,
}

/// Manager for coherence worker lifecycle
#[derive(Debug)]
pub struct CoherenceWorkerManager<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned, V: CacheValue + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned> {
    config: ProtocolConfiguration,
    worker_handle: Option<JoinHandle<()>>,
    _phantom: PhantomData<(K, V)>,
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned + 'static, V: CacheValue + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned + 'static> CoherenceWorkerManager<K, V> {
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

        let worker = CoherenceWorker::new(
            self.config.clone(),
            request_rx,
            response_tx,
            shutdown_rx,
        );

        let handle = thread::spawn(move || {
            worker.run();
        });

        self.worker_handle = Some(handle);

        Ok(CoherenceSender {
            request_tx,
            response_rx,
            shutdown_tx,
            response_buffer: Arc::new(Mutex::new(HashMap::new())),
            buffer_limit: 1000, // Prevent unbounded growth
        })
    }

    /// Shutdown worker gracefully
    pub fn shutdown(self) -> Result<(), CoherenceError> {
        if let Some(handle) = self.worker_handle {
            handle.join().map_err(|_| CoherenceError::CommunicationFailure)?;
        }
        Ok(())
    }
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned, V: CacheValue + Default + bincode::Encode + bincode::Decode<()> + serde::Serialize + serde::de::DeserializeOwned> CoherenceSender<K, V> {
    /// Send request to worker
    pub fn send_request(&self, request: CoherenceRequest<K, V>) -> Result<(), CoherenceError> {
        self.request_tx.send(request).map_err(|_| CoherenceError::ChannelClosed)
    }

    /// Receive response with timeout
    pub fn receive_response(&self, request_id: u64, timeout: Duration) -> Result<CoherenceResponse<K, V>, CoherenceError> {
        // Check buffer first for previously received responses
        if let Ok(mut buffer) = self.response_buffer.lock()
            && let Some(response) = buffer.remove(&request_id) {
                return Ok(response);
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
                        if let Ok(mut buffer) = self.response_buffer.lock() {
                            // Prevent unbounded growth by limiting buffer size
                            if buffer.len() < self.buffer_limit {
                                buffer.insert(response_id, response);
                            } else {
                                // Buffer full - remove oldest entry to make room
                                if let Some(&oldest_id) = buffer.keys().min() {
                                    buffer.remove(&oldest_id);
                                }
                                buffer.insert(response_id, response);
                            }
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
            CoherenceResponse::ReadSuccess { request_id, .. } |
            CoherenceResponse::WriteSuccess { request_id, .. } |
            CoherenceResponse::Statistics { request_id, .. } |
            CoherenceResponse::SerializeSuccess { request_id, .. } |
            CoherenceResponse::Error { request_id, .. } => *request_id,
        }
    }
    
    /// Initiate graceful shutdown of the worker thread
    pub fn shutdown(&self) -> Result<(), CoherenceError> {
        self.shutdown_tx.send(()).map_err(|_| CoherenceError::ChannelClosed)
    }
    
    /// Clear response buffer to prevent memory leaks
    #[allow(dead_code)] // MESI coherence - used in worker maintenance and memory management
    pub fn clear_buffer(&self) -> Result<usize, CoherenceError> {
        if let Ok(mut buffer) = self.response_buffer.lock() {
            let cleared_count = buffer.len();
            buffer.clear();
            Ok(cleared_count)
        } else {
            Err(CoherenceError::CommunicationFailure)
        }
    }
}