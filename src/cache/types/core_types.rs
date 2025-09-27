//! Core type definitions for the cache system.

/// Cache operation result type
pub type CacheResult<T> = Result<T, crate::cache::traits::CacheOperationError>;

/// Alert severity levels
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[repr(u8)]
pub enum AlertSeverity {
    Info = 0,
    Low = 1,
    Medium = 2,
    Warning = 3,
    High = 4,
    Error = 5,
    Critical = 6,
    Emergency = 7,
}

impl From<AlertSeverity> for u8 {
    fn from(severity: AlertSeverity) -> u8 {
        severity as u8
    }
}

impl AlertSeverity {
    /// Get numeric priority value
    pub fn priority(&self) -> u8 {
        *self as u8
    }

    /// Check if severity is critical level
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Critical | Self::Emergency)
    }

    /// Check if severity requires immediate action
    pub fn requires_immediate_action(&self) -> bool {
        matches!(self, Self::Critical | Self::Emergency | Self::Error)
    }
}

// CacheOperationError moved to canonical location: crate::cache::traits::types_and_enums

// PrecisionTimer moved to canonical location: crate::cache::types::performance::timer::PrecisionTimer

// CacheEntry moved to canonical location: crate::cache::traits::cache_entry::CacheEntry
// Use the comprehensive implementation for all cache entry needs

/// Batch operation request
#[allow(dead_code)] // Core types - batch request structure for batch operation coordination
#[derive(Debug, Clone)]
pub struct BatchRequest<K, V> {
    pub operations: Vec<BatchOperation<K, V>>,
    pub timeout_ms: Option<u64>,
}

/// Individual batch operation
#[allow(dead_code)] // Core types - batch operation enumeration for batch processing
#[derive(Debug, Clone)]
pub enum BatchOperation<K, V> {
    Get(K),
    Put(K, V),
    Remove(K),
}

// BatchResult moved to canonical location: crate::cache::types::batch_operations::BatchResult
// Use the enhanced canonical implementation with HashMap-based indexing, TimedResult wrapper,
// comprehensive timing infrastructure, and production-ready batch operation analysis

// TierStatistics moved to canonical location: crate::cache::types::statistics::tier_stats::TierStatistics

/// Get current timestamp in nanoseconds
#[allow(dead_code)] // Core types - timestamp utility function for operation timing
pub fn timestamp_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
