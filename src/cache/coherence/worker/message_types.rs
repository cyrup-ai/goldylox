//! Message types for coherence worker communication
//!
//! Request/response types for all coherence operations via channels

use tokio::sync::oneshot;

use crate::cache::coherence::CoherenceStatisticsSnapshot;
use crate::cache::coherence::communication::{CoherenceError, ReadResponse, WriteResponse};
use crate::cache::coherence::data_structures::CacheTier;
use crate::cache::traits::cache_entry::SerializationEnvelope;
use crate::cache::traits::{CacheKey, CacheValue};

/// Request message for coherence worker
#[derive(Debug)]
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
        response: oneshot::Sender<Result<ReadResponse, CoherenceError>>,
    },
    /// Request coherent write operation  
    Write {
        key: K,
        data: V,
        requesting_tier: CacheTier,
        response: oneshot::Sender<Result<WriteResponse, CoherenceError>>,
    },
    /// Request statistics
    #[allow(dead_code)]
    // MESI coherence - used in coherence statistics collection and monitoring
    GetStatistics {
        response: oneshot::Sender<CoherenceStatisticsSnapshot>,
    },
    /// Request serialization envelope creation
    Serialize {
        key: K,
        value: V,
        target_tier: CacheTier,
        response: oneshot::Sender<Result<Box<SerializationEnvelope<K, V>>, CoherenceError>>,
    },
    /// Record read access for coherence tracking
    RecordRead {
        key: K,
        tier: CacheTier,
        response: oneshot::Sender<Result<(), CoherenceError>>,
    },
    /// Record write access for coherence tracking
    RecordWrite {
        key: K,
        data: V,
        tier: CacheTier,
        response: oneshot::Sender<Result<(), CoherenceError>>,
    },
    /// Record prefetch access for coherence tracking
    RecordPrefetch {
        key: K,
        tier: CacheTier,
        response: oneshot::Sender<Result<(), CoherenceError>>,
    },
}
