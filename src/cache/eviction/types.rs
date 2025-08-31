//! Common types and enums for cache eviction policies
//!
//! This module contains shared data structures used across all eviction policy implementations.



use crate::cache::coherence::CacheTier;
pub use crate::cache::traits::AccessType;
use crate::cache::traits::CacheKey;

/// Policy type enumeration for adaptive switching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyType {
    AdaptiveLRU,
    AdaptiveLFU,
    TwoQueue,
    ARC,
    MLPredictive,
}

/// Write strategy enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStrategy {
    WriteThrough,
    WriteBack,
    WriteBehind,
}

/// Consistency level requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyLevel {
    Eventual,
    Strong,
    Causal,
}

/// Enhanced access event record for pattern analysis
#[derive(Debug, Clone)]
#[derive(serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
#[serde(bound(serialize = "K: serde::Serialize", deserialize = "K: serde::de::DeserializeOwned"))]
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
        Self::WriteThrough
    }
}

impl Default for ConsistencyLevel {
    fn default() -> Self {
        Self::Eventual
    }
}

impl<K: CacheKey> AccessEvent<K> {
    /// Create new access event with current timestamp
    pub fn new(key: K, access_type: AccessType, tier: CacheTier, hit: bool) -> Self {
        static EVENT_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        
        Self {
            event_id: EVENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
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
    pub fn with_slot_index(mut self, slot_index: usize) -> Self {
        self.slot_index = Some(slot_index);
        self
    }
    
    /// Create event with performance metrics
    pub fn with_metrics(mut self, latency_ns: u64, entry_size: usize) -> Self {
        self.latency_ns = latency_ns;
        self.entry_size = entry_size;
        self
    }

    /// Create basic access event (backward compatibility)  
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
