//! Message types for coherence worker communication
//!
//! Request/response types for all coherence operations via channels

use crate::cache::coherence::CoherenceStatisticsSnapshot;
use crate::cache::coherence::communication::{CoherenceError, ReadResponse, WriteResponse};
use crate::cache::coherence::data_structures::CacheTier;
use crate::cache::traits::cache_entry::SerializationEnvelope;
use crate::cache::traits::{CacheKey, CacheValue};

/// Request message for coherence worker
#[derive(Debug, Clone)]
pub enum CoherenceRequest<
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
    /// Request coherent read operation
    Read {
        key: K,
        requesting_tier: CacheTier,
        request_id: u64,
    },
    /// Request coherent write operation  
    Write {
        key: K,
        data: V,
        requesting_tier: CacheTier,
        request_id: u64,
    },
    /// Request statistics
    #[allow(dead_code)]
    // MESI coherence - used in coherence statistics collection and monitoring
    GetStatistics { request_id: u64 },
    /// Request serialization envelope creation
    Serialize {
        key: K,
        value: V,
        target_tier: CacheTier,
        request_id: u64,
    },
    /// Record read access for coherence tracking
    RecordRead {
        key: K,
        tier: CacheTier,
        request_id: u64,
    },
    /// Record write access for coherence tracking
    RecordWrite {
        key: K,
        data: V,
        tier: CacheTier,
        request_id: u64,
    },
    /// Record prefetch access for coherence tracking
    RecordPrefetch {
        key: K,
        tier: CacheTier,
        request_id: u64,
    },
}

/// Response message from coherence worker
#[derive(Debug, Clone)]
pub enum CoherenceResponse<
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
    /// Successful read response
    ReadSuccess {
        request_id: u64,
        response: ReadResponse,
    },
    /// Successful write response
    WriteSuccess {
        request_id: u64,
        response: WriteResponse,
    },
    /// Statistics response
    Statistics {
        request_id: u64,
        #[allow(dead_code)]
        // MESI coherence - used in coherence statistics reporting and telemetry
        statistics: CoherenceStatisticsSnapshot,
    },
    /// Successful serialization response
    SerializeSuccess {
        request_id: u64,
        envelope: Box<SerializationEnvelope<K, V>>,
    },
    /// Successful access record response
    AccessRecorded { request_id: u64 },
    /// Error response for any operation
    Error {
        request_id: u64,
        error: CoherenceError,
    },
}
