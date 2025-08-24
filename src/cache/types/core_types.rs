//! Core type definitions for the cache system.

use std::time::Instant;

/// Cache operation result type
pub type CacheResult<T> = Result<T, crate::cache::traits::CacheOperationError>;

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AlertSeverity {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
    Warning = 4,
}

impl From<AlertSeverity> for u8 {
    fn from(severity: AlertSeverity) -> u8 {
        severity as u8
    }
}

// CacheOperationError moved to canonical location: crate::cache::traits::types_and_enums

/// Precision timer for performance measurements
#[derive(Debug, Clone)]
pub struct PrecisionTimer {
    start_time: Instant,
    label: String,
}

impl PrecisionTimer {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            start_time: Instant::now(),
            label: label.into(),
        }
    }

    pub fn elapsed_nanos(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }

    pub fn elapsed_micros(&self) -> u64 {
        self.start_time.elapsed().as_micros() as u64
    }
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<V> {
    pub value: V,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub size_bytes: usize,
}

/// Batch operation request
#[derive(Debug, Clone)]
pub struct BatchRequest<K, V> {
    pub operations: Vec<BatchOperation<K, V>>,
    pub timeout_ms: Option<u64>,
}

/// Individual batch operation
#[derive(Debug, Clone)]
pub enum BatchOperation<K, V> {
    Get(K),
    Put(K, V),
    Remove(K),
}

/// Batch operation result
#[derive(Debug, Clone)]
pub struct BatchResult<V> {
    pub results: Vec<CacheResult<Option<V>>>,
    pub completed_count: usize,
    pub failed_count: usize,
}

/// Tier statistics
#[derive(Debug, Clone)]
pub struct TierStatistics {
    pub hit_count: u64,
    pub miss_count: u64,
    pub total_size_bytes: u64,
    pub entry_count: u64,
    pub avg_access_time_ns: u64,
}

/// Get current timestamp in nanoseconds
pub fn timestamp_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
