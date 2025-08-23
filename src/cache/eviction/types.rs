//! Common types and enums for cache eviction policies
//!
//! This module contains shared data structures used across all eviction policy implementations.

use std::time::Instant;

use super::super::coherence::CacheTier;
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

/// Access event record for pattern analysis
#[derive(Debug, Clone)]
pub struct AccessEvent<K: CacheKey> {
    /// Cache key that was accessed
    pub key: K,
    /// Access timestamp
    pub timestamp: u64, // Nanoseconds since epoch
    /// Access type (read/write)
    pub access_type: AccessType,
    /// Cache tier where access occurred
    pub tier: CacheTier,
}

/// Prefetch request with priority and context
#[derive(Debug, Clone)]
pub struct PrefetchRequest<K: CacheKey> {
    /// Key to prefetch
    pub key: K,
    /// Predicted access time
    pub predicted_access: u64, // Nanoseconds since epoch
    /// Prefetch priority (0-10)
    pub priority: u8,
    /// Confidence in prediction (0.0-1.0)
    pub confidence: f32,
    /// Request creation time
    pub created_at: Instant,
}

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
    /// Create new access event
    pub fn new(key: K, access_type: AccessType, tier: CacheTier) -> Self {
        Self {
            key,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            access_type,
            tier,
        }
    }
}

impl<K: CacheKey> PrefetchRequest<K> {
    /// Create new prefetch request
    pub fn new(key: K, predicted_access: u64, priority: u8, confidence: f32) -> Self {
        Self {
            key,
            predicted_access,
            priority: priority.min(10),
            confidence: confidence.clamp(0.0, 1.0),
            created_at: Instant::now(),
        }
    }

    /// Check if request has expired based on age
    pub fn is_expired(&self, max_age: std::time::Duration) -> bool {
        self.created_at.elapsed() > max_age
    }

    /// Get request age in milliseconds
    pub fn age_ms(&self) -> u64 {
        self.created_at.elapsed().as_millis() as u64
    }
}
