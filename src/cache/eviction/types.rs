//! Common types and enums for cache eviction policies
//!
//! This module contains shared data structures used across all eviction policy implementations.

use crate::cache::coherence::CacheTier;
pub(crate) use crate::cache::traits::AccessType;
use crate::cache::traits::CacheKey;

/// Policy type enumeration for adaptive switching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyType {
    AdaptiveLRU,
    #[allow(dead_code)] // Eviction policies - adaptive LFU used in policy switching
    AdaptiveLFU,
    #[allow(dead_code)] // Eviction policies - two queue used in policy switching
    TwoQueue,
    #[allow(dead_code)] // Eviction policies - Arc used in policy switching
    Arc,
    #[allow(dead_code)] // Eviction policies - ML predictive used in policy switching
    MLPredictive,
}

/// Write operation execution strategy
/// 
/// Determines when and how write operations are persisted to storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriteStrategy {
    /// Write-through: immediate persistence
    /// 
    /// Highest latency, strongest consistency. Used for critical hot data
    /// that must be durable immediately.
    Immediate,
    
    /// Buffered: batch writes for throughput
    /// 
    /// Balanced latency/consistency. Writes are batched and flushed
    /// periodically. Default strategy for most workloads.
    Buffered,
    
    /// Deferred: schedule during maintenance windows
    /// 
    /// Lowest latency, eventual consistency. Writes are queued and
    /// executed during background maintenance. Used for cold tier data.
    Deferred,
}

/// Consistency level requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyLevel {
    Eventual,
    #[allow(dead_code)] // Write policies - strong consistency used in consistency configuration
    Strong,
    #[allow(dead_code)] // Write policies - causal consistency used in consistency configuration
    Causal,
}

/// Enhanced access event record for pattern analysis
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
#[serde(bound(
    serialize = "K: serde::Serialize",
    deserialize = "K: serde::de::DeserializeOwned"
))]
pub struct AccessEvent<K: CacheKey> {
    /// Unique event identifier
    pub event_id: u64,
    /// Cache key that was accessed
    pub key: K,
    /// Access timestamp
    pub timestamp: u64, // Nanoseconds since epoch
    /// Access type (read/write)
    pub access_type: AccessType,
    /// Cache tier where access occurred
    pub tier: CacheTier,
    /// Whether access was a hit
    pub hit: bool,
    /// Optional slot index for hot tier compatibility
    pub slot_index: Option<usize>,
    /// Access latency in nanoseconds
    pub latency_ns: u64,
    /// Entry size in bytes
    pub entry_size: usize,
}

// PrefetchRequest moved to canonical location: crate::cache::tier::hot::prefetch::types::PrefetchRequest
pub use crate::cache::tier::hot::prefetch::types::PrefetchRequest;

// AccessType moved to canonical location: crate::cache::traits::types_and_enums

impl Default for PolicyType {
    fn default() -> Self {
        Self::AdaptiveLRU
    }
}

impl Default for WriteStrategy {
    fn default() -> Self {
        Self::Buffered  // Balanced default for most workloads
    }
}

impl Default for ConsistencyLevel {
    fn default() -> Self {
        Self::Eventual
    }
}

impl<K: CacheKey> AccessEvent<K> {
    /// Create new access event with current timestamp
    #[allow(dead_code)] // Eviction policies - new access event used in access pattern tracking
    pub fn new(
        key: K,
        access_type: AccessType,
        tier: CacheTier,
        hit: bool,
        event_counter: &std::sync::atomic::AtomicU64,
    ) -> Self {
        Self {
            event_id: event_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            key,
            timestamp: crate::cache::types::timestamp_nanos(std::time::Instant::now()),
            access_type,
            tier,
            hit,
            slot_index: None,
            latency_ns: 0,
            entry_size: 0,
        }
    }

    /// Create event with slot index for hot tier
    #[allow(dead_code)] // Eviction policies - with slot index used in hot tier event tracking
    pub fn with_slot_index(mut self, slot_index: usize) -> Self {
        self.slot_index = Some(slot_index);
        self
    }

    /// Create event with performance metrics
    #[allow(dead_code)] // Eviction policies - with metrics used in performance event tracking
    pub fn with_metrics(mut self, latency_ns: u64, entry_size: usize) -> Self {
        self.latency_ns = latency_ns;
        self.entry_size = entry_size;
        self
    }

    /// Create basic access event (backward compatibility)  
    #[allow(dead_code)] // Eviction policies - new simple used in backward compatibility
    pub fn new_simple(key: K, access_type: AccessType, tier: CacheTier) -> Self {
        Self {
            event_id: 0, // No event ID for simple version
            key,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            access_type,
            tier,
            hit: false, // Default to miss for simple version
            slot_index: None,
            latency_ns: 0,
            entry_size: 0,
        }
    }
}

// PrefetchRequest implementation moved to canonical location:
// crate::cache::tier::hot::prefetch::types::PrefetchRequest
// The canonical version provides enhanced functionality with metadata and pattern detection
