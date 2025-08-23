//! Inter-tier communication system for cache coherence protocol
//!
//! This module implements the communication hub and message types for coordinating
//! coherence operations between Hot, Warm, and Cold cache tiers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};

use super::data_structures::{CacheTier, CoherenceKey, InvalidationReason};
use crate::cache::traits::{CacheKey, CacheValue};

/// Inter-tier communication hub for coherence messages
#[derive(Debug)]
pub struct CommunicationHub<K: CacheKey, V: CacheValue> {
    /// Hot tier communication channel
    pub hot_tx: Sender<CoherenceMessage<K, V>>,
    pub hot_rx: Receiver<CoherenceMessage<K, V>>,
    /// Warm tier communication channel
    pub warm_tx: Sender<CoherenceMessage<K, V>>,
    pub warm_rx: Receiver<CoherenceMessage<K, V>>,
    /// Cold tier communication channel
    pub cold_tx: Sender<CoherenceMessage<K, V>>,
    pub cold_rx: Receiver<CoherenceMessage<K, V>>,
    /// Broadcast channel for global notifications
    pub broadcast_tx: Sender<CoherenceMessage<K, V>>,
    pub broadcast_rx: Receiver<CoherenceMessage<K, V>>,
    /// Message statistics
    pub message_stats: MessageStatistics,
}

/// Coherence message types for inter-tier communication
#[derive(Debug, Clone)]
pub enum CoherenceMessage<K: CacheKey, V: CacheValue> {
    /// Request exclusive access for write
    RequestExclusive {
        key: CoherenceKey<K>,
        requester_tier: CacheTier,
        version: u64,
        timestamp_ns: u64,
    },
    /// Request shared access for read
    RequestShared {
        key: CoherenceKey<K>,
        requester_tier: CacheTier,
        version: u64,
        timestamp_ns: u64,
    },
    /// Grant exclusive access
    GrantExclusive {
        key: CoherenceKey<K>,
        target_tier: CacheTier,
        version: u64,
        timestamp_ns: u64,
    },
    /// Grant shared access
    GrantShared {
        key: CoherenceKey<K>,
        target_tier: CacheTier,
        version: u64,
        timestamp_ns: u64,
    },
    /// Invalidate cache line
    Invalidate {
        key: CoherenceKey<K>,
        target_tier: CacheTier,
        reason: InvalidationReason,
        sequence: u32,
        timestamp_ns: u64,
    },
    /// Write-back notification
    WriteBack {
        key: CoherenceKey<K>,
        source_tier: CacheTier,
        data_version: u64,
        timestamp_ns: u64,
    },
    /// Data transfer between tiers
    DataTransfer {
        key: CoherenceKey<K>,
        source_tier: CacheTier,
        target_tier: CacheTier,
        data: Arc<V>,
        version: u64,
        timestamp_ns: u64,
    },
}

/// Message statistics for monitoring communication
#[derive(Debug)]
pub struct MessageStatistics {
    /// Total messages sent
    pub messages_sent: AtomicU64,
    /// Messages received
    pub messages_received: AtomicU64,
    /// Failed message deliveries
    pub failed_deliveries: AtomicU64,
    /// Average message latency in nanoseconds
    pub avg_latency_ns: AtomicU64,
    /// Peak message queue depth
    pub peak_queue_depth: AtomicU64,
}

/// Response types for coherence operations
#[derive(Debug, Clone, Copy)]
pub enum ReadResponse {
    Hit,
    SharedGranted,
    Miss,
}

#[derive(Debug, Clone, Copy)]
pub enum WriteResponse {
    Success,
    Conflict,
}

#[derive(Debug, Clone, Copy)]
pub enum ExclusiveResponse {
    Granted,
    Conflict,
}

/// Coherence operation errors
#[derive(Debug)]
pub enum CoherenceError {
    InvalidStateTransition {
        from: super::data_structures::MesiState,
        to: super::data_structures::MesiState,
    },
    CommunicationFailure,
    CacheLineNotFound,
    TimeoutExpired,
    ProtocolViolation,
    InvalidMessage,
    ChannelClosed,
    SerializationFailed(String),
    TierAccessFailed(String),
    WriteConflict,
    // Connect to existing type-safe deserialization validation
    UnsupportedSchemaVersion(u32),
    ChecksumMismatch { expected: u64, actual: u64 },
}

impl<K: CacheKey, V: CacheValue> CommunicationHub<K, V> {
    pub fn new() -> Self {
        let (hot_tx, hot_rx) = unbounded();
        let (warm_tx, warm_rx) = unbounded();
        let (cold_tx, cold_rx) = unbounded();
        let (broadcast_tx, broadcast_rx) = unbounded();

        Self {
            hot_tx,
            hot_rx,
            warm_tx,
            warm_rx,
            cold_tx,
            cold_rx,
            broadcast_tx,
            broadcast_rx,
            message_stats: MessageStatistics::new(),
        }
    }

    /// Send message to specific tier
    pub fn send_to_tier(
        &self,
        tier: CacheTier,
        message: CoherenceMessage<K, V>,
    ) -> Result<(), CoherenceError> {
        let result = match tier {
            CacheTier::Hot => self.hot_tx.send(message),
            CacheTier::Warm => self.warm_tx.send(message),
            CacheTier::Cold => self.cold_tx.send(message),
        };

        match result {
            Ok(()) => {
                self.message_stats
                    .messages_sent
                    .fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(_) => {
                self.message_stats
                    .failed_deliveries
                    .fetch_add(1, Ordering::Relaxed);
                Err(CoherenceError::CommunicationFailure)
            }
        }
    }

    /// Broadcast message to all tiers
    pub fn broadcast(&self, message: CoherenceMessage<K, V>) -> Result<(), CoherenceError> {
        match self.broadcast_tx.send(message) {
            Ok(()) => {
                self.message_stats
                    .messages_sent
                    .fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(_) => {
                self.message_stats
                    .failed_deliveries
                    .fetch_add(1, Ordering::Relaxed);
                Err(CoherenceError::CommunicationFailure)
            }
        }
    }

    /// Receive message from specific tier (non-blocking)
    pub fn try_receive_from_tier(&self, tier: CacheTier) -> Option<CoherenceMessage<K, V>> {
        let result = match tier {
            CacheTier::Hot => self.hot_rx.try_recv(),
            CacheTier::Warm => self.warm_rx.try_recv(),
            CacheTier::Cold => self.cold_rx.try_recv(),
        };

        match result {
            Ok(message) => {
                self.message_stats
                    .messages_received
                    .fetch_add(1, Ordering::Relaxed);
                Some(message)
            }
            Err(_) => None,
        }
    }

    /// Receive broadcast message (non-blocking)
    pub fn try_receive_broadcast(&self) -> Option<CoherenceMessage<K, V>> {
        match self.broadcast_rx.try_recv() {
            Ok(message) => {
                self.message_stats
                    .messages_received
                    .fetch_add(1, Ordering::Relaxed);
                Some(message)
            }
            Err(_) => None,
        }
    }

    /// Get communication statistics
    pub fn get_statistics(&self) -> MessageStatisticsSnapshot {
        MessageStatisticsSnapshot {
            messages_sent: self.message_stats.messages_sent.load(Ordering::Relaxed),
            messages_received: self.message_stats.messages_received.load(Ordering::Relaxed),
            failed_deliveries: self.message_stats.failed_deliveries.load(Ordering::Relaxed),
            avg_latency_ns: self.message_stats.avg_latency_ns.load(Ordering::Relaxed),
            peak_queue_depth: self.message_stats.peak_queue_depth.load(Ordering::Relaxed),
        }
    }
}

impl MessageStatistics {
    pub fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            failed_deliveries: AtomicU64::new(0),
            avg_latency_ns: AtomicU64::new(0),
            peak_queue_depth: AtomicU64::new(0),
        }
    }

    /// Update latency statistics
    pub fn update_latency(&self, latency_ns: u64) {
        // Simple exponential moving average
        let current = self.avg_latency_ns.load(Ordering::Relaxed);
        let new_avg = if current == 0 {
            latency_ns
        } else {
            (current * 7 + latency_ns) / 8 // 7/8 weight to previous, 1/8 to new
        };
        self.avg_latency_ns.store(new_avg, Ordering::Relaxed);
    }

    /// Update queue depth statistics
    pub fn update_queue_depth(&self, depth: u64) {
        let current_peak = self.peak_queue_depth.load(Ordering::Relaxed);
        if depth > current_peak {
            self.peak_queue_depth.store(depth, Ordering::Relaxed);
        }
    }
}

/// Snapshot of message statistics for monitoring
#[derive(Debug, Clone, Copy)]
pub struct MessageStatisticsSnapshot {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub failed_deliveries: u64,
    pub avg_latency_ns: u64,
    pub peak_queue_depth: u64,
}

impl<K: CacheKey, V: CacheValue> CoherenceMessage<K, V> {
    /// Get the coherence key from any message type
    pub fn key(&self) -> &CoherenceKey<K> {
        match self {
            CoherenceMessage::RequestExclusive { key, .. } => key,
            CoherenceMessage::RequestShared { key, .. } => key,
            CoherenceMessage::GrantExclusive { key, .. } => key,
            CoherenceMessage::GrantShared { key, .. } => key,
            CoherenceMessage::Invalidate { key, .. } => key,
            CoherenceMessage::WriteBack { key, .. } => key,
            CoherenceMessage::DataTransfer { key, .. } => key,
        }
    }

    /// Get the timestamp from any message type
    pub fn timestamp_ns(&self) -> u64 {
        match self {
            CoherenceMessage::RequestExclusive { timestamp_ns, .. } => *timestamp_ns,
            CoherenceMessage::RequestShared { timestamp_ns, .. } => *timestamp_ns,
            CoherenceMessage::GrantExclusive { timestamp_ns, .. } => *timestamp_ns,
            CoherenceMessage::GrantShared { timestamp_ns, .. } => *timestamp_ns,
            CoherenceMessage::Invalidate { timestamp_ns, .. } => *timestamp_ns,
            CoherenceMessage::WriteBack { timestamp_ns, .. } => *timestamp_ns,
            CoherenceMessage::DataTransfer { timestamp_ns, .. } => *timestamp_ns,
        }
    }

    /// Check if message is a request type
    pub fn is_request(&self) -> bool {
        matches!(
            self,
            CoherenceMessage::RequestExclusive { .. } | CoherenceMessage::RequestShared { .. }
        )
    }

    /// Check if message is a grant type
    pub fn is_grant(&self) -> bool {
        matches!(
            self,
            CoherenceMessage::GrantExclusive { .. } | CoherenceMessage::GrantShared { .. }
        )
    }

    /// Check if message involves data transfer
    pub fn has_data(&self) -> bool {
        matches!(self, CoherenceMessage::DataTransfer { .. })
    }
}
