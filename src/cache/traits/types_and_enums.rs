//! Type definitions, enums, and data structures for cache system
//!
//! This module contains all the supporting types, enumerations, and data structures
//! that provide configuration and metadata for the cache system.

use std::time::{Duration, Instant};

use super::supporting_types::CompressionAlgorithm;

/// Tier affinity hints for intelligent placement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierAffinity {
    /// Automatic tier selection based on patterns
    Auto,
    /// Prefer hot tier (thread-local)
    Hot,
    /// Prefer warm tier (shared)
    Warm,
    /// Prefer cold tier (persistent)
    Cold,
    /// Multi-tier replication
    Replicated,
}

/// Placement hints for cache insertion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementHint {
    /// Standard placement using eviction policy
    Standard,
    /// High priority placement
    Priority,
    /// Temporary placement (short-lived)
    Temporary,
    /// Pin to specific tier
    Pin(TierLocation),
}

/// Tier location enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TierLocation {
    Hot,
    Warm,
    Cold,
}

/// Access type classification for pattern recognition and operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// Sequential access pattern
    Sequential,
    /// Random access pattern
    Random,
    /// Temporal locality access
    Temporal,
    /// Spatial locality access
    Spatial,
    /// Prefetch access
    Prefetch,
    /// Basic read operation
    Read,
    /// Basic write operation
    Write,
    /// Cache hit
    Hit,
    /// Cache miss
    Miss,
    /// Tier promotion operation
    Promotion,
    /// Tier demotion operation
    Demotion,
    /// Sequential read operation
    SequentialRead,
    /// Random read operation
    RandomRead,
    /// Sequential write operation
    SequentialWrite,
    /// Read-modify-write operation
    ReadModifyWrite,
    /// Prefetch hit operation
    PrefetchHit,
}

/// Eviction reason classification (comprehensive superset)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionReason {
    /// Capacity limit reached
    CapacityLimit,
    /// Time-based expiration
    Expired,
    /// Memory pressure
    MemoryPressure,
    /// Manual eviction
    Manual,
    /// Policy-based decision
    Policy,
    /// Advanced policy decision with reasoning
    PolicyDecision,
    /// Least recently used algorithm
    LeastRecentlyUsed,
    /// Least frequently used algorithm
    LeastFrequentlyUsed,
    /// Adaptive replacement cache decision
    AdaptiveReplacement,
    /// Machine learning prediction
    MachineLearning,
    /// Low utility score
    LowUtility,
    /// Size optimization (large entries)
    SizeOptimization,
    /// Cache pressure optimization
    CachePressure,
}

/// Compression hint for value storage (consolidated superset)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionHint {
    /// Automatic compression decision
    Auto,
    /// Force compression
    Force,
    /// Prefer compression (soft hint)
    Prefer,
    /// Disable compression
    Disable,
    /// Use specific algorithm
    Algorithm(CompressionAlgorithm),
}

/// Priority class enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityClass {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Volatility level for cache decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolatilityLevel {
    /// Rarely changes
    Stable,
    /// Changes occasionally  
    Moderate,
    /// Changes frequently
    Volatile,
    /// Changes constantly
    HighlyVolatile,
}

/// Temporal access pattern classification (sophisticated superset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TemporalPattern {
    /// Steady access rate
    Steady,
    /// Burst access pattern
    Bursty,
    // REMOVED: Burst alias for backward compatibility
    // Users must now use Bursty instead of Burst
    /// High-frequency burst pattern
    BurstyHigh,
    /// Periodic access pattern
    Periodic,
    /// Declining access rate
    Declining,
    /// Write-heavy pattern
    WriteHeavy,
    /// Irregular access pattern
    Irregular,
    /// Random access pattern
    Random,
    /// Sequential access pattern
    Sequential,
}

/// Selection reason for eviction candidates (consolidated superset)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionReason {
    LeastRecentlyUsed,
    LeastFrequentlyUsed,
    /// High frequency selection (from types.rs)
    HighFrequency,
    /// Recent access selection (from types.rs)
    RecentAccess,
    Expired,
    LowPriority,
    LargeSize,
    PolicyDecision,
    /// Policy pattern match (from types.rs)
    PolicyMatch,
}

/// Error category classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Resource exhaustion
    Resource,
    /// Serialization/deserialization
    Serialization,
    /// IO operations
    Io,
    /// Lock contention (should not occur in lock-free impl)
    Concurrency,
    /// Configuration issues
    Configuration,
    /// Timing-related errors
    Timing,
    /// Invalid state errors
    InvalidState,
}

/// Recovery hint for error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryHint {
    /// Retry immediately
    RetryImmediate,
    /// Retry with backoff
    RetryBackoff,
    /// Clear cache and retry
    ClearAndRetry,
    /// Use fallback mechanism
    Fallback,
    /// Manual intervention required
    Manual,
}

/// Cache operation error types (from types.rs - comprehensive error handling)
#[derive(Debug, Clone, PartialEq)]
pub enum CacheOperationError {
    KeyNotFound,
    SerializationError(String),
    DeserializationError(String),
    StorageError(String),
    MemoryLimitExceeded,
    TimeoutError,
    InvalidConfiguration(String),
    TierError(String),
    Io(String),
    CoherenceError,
    TierOperationFailed,
    InvalidState(String),
    ResourceExhausted(String),
    ConfigurationError(String),
    Corruption(String),
    NotFound,
    InvalidOperation,
    InternalError,
    InitializationFailed,
    OperationFailed,
    InvalidArgument(String),
    ConcurrentAccess(String),
    TimingError(String),
}

impl std::fmt::Display for CacheOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheOperationError::KeyNotFound => write!(f, "Key not found in cache"),
            CacheOperationError::SerializationError(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
            CacheOperationError::DeserializationError(msg) => {
                write!(f, "Deserialization error: {}", msg)
            }
            CacheOperationError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            CacheOperationError::MemoryLimitExceeded => write!(f, "Memory limit exceeded"),
            CacheOperationError::TimeoutError => write!(f, "Operation timed out"),
            CacheOperationError::InvalidConfiguration(msg) => {
                write!(f, "Invalid configuration: {}", msg)
            }
            CacheOperationError::TierError(msg) => write!(f, "Tier operation error: {}", msg),
            CacheOperationError::Io(msg) => write!(f, "I/O error: {}", msg),
            CacheOperationError::CoherenceError => write!(f, "Cache coherence error"),
            CacheOperationError::TierOperationFailed => write!(f, "Tier operation failed"),
            CacheOperationError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            CacheOperationError::ResourceExhausted(msg) => write!(f, "Resource exhausted: {}", msg),
            CacheOperationError::ConfigurationError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            CacheOperationError::Corruption(msg) => write!(f, "Data corruption: {}", msg),
            CacheOperationError::NotFound => write!(f, "Not found"),
            CacheOperationError::InvalidOperation => write!(f, "Invalid operation"),
            CacheOperationError::InternalError => write!(f, "Internal error"),
            CacheOperationError::InitializationFailed => write!(f, "Initialization failed"),
            CacheOperationError::OperationFailed => write!(f, "Operation failed"),
            CacheOperationError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            CacheOperationError::ConcurrentAccess(msg) => {
                write!(f, "Concurrent access error: {}", msg)
            }
            CacheOperationError::TimingError(msg) => write!(f, "Timing error: {}", msg),
        }
    }
}

impl std::error::Error for CacheOperationError {}

impl CacheOperationError {
    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::InvalidState(msg.into())
    }

    pub fn resource_exhausted(msg: impl Into<String>) -> Self {
        Self::ResourceExhausted(msg.into())
    }

    pub fn configuration_error(msg: impl Into<String>) -> Self {
        Self::ConfigurationError(msg.into())
    }

    pub fn io_error(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }

    pub fn io_failed(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }

    pub fn serialization_failed(msg: impl Into<String>) -> Self {
        Self::SerializationError(msg.into())
    }

    pub fn concurrency_error(msg: impl Into<String>) -> Self {
        Self::TierError(msg.into())
    }

    pub fn initialization_failed(msg: impl Into<String>) -> Self {
        Self::InvalidConfiguration(msg.into())
    }

    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }
}

/// Policy action types (from types.rs - ML and policy engine support)
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyAction {
    Promote,
    Demote,
    Evict,
    Retain,
    Prefetch,
}

/// Capacity information structure
#[derive(Debug, Clone)]
pub struct CapacityInfo {
    /// Current entry count
    pub current: usize,
    /// Maximum entries
    pub maximum: usize,
    /// Memory usage in bytes
    pub memory_bytes: usize,
    /// Memory limit in bytes
    pub memory_limit: usize,
}

/// Maintenance report structure
#[derive(Debug, Clone)]
pub struct MaintenanceReport {
    /// Entries cleaned up
    pub cleaned_entries: usize,
    /// Bytes reclaimed
    pub bytes_reclaimed: usize,
    /// Time spent in maintenance
    pub duration: Duration,
    /// Issues found and fixed
    pub issues_resolved: usize,
}

/// Performance metrics for policy optimization
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Overall hit rate
    pub hit_rate: f64,
    /// Average access time
    pub avg_access_time_ns: u64,
    /// Operations per second
    pub ops_per_second: f64,
    /// Memory efficiency ratio
    pub memory_efficiency: f64,
}

/// Cache error context (from types.rs - debugging support)
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub tier: Option<String>,
    pub key_hash: Option<u64>,
    pub timestamp: Instant,
}

/// Monitoring metrics (from types.rs - runtime monitoring)
#[derive(Debug, Clone)]
pub struct MonitoringMetrics {
    pub hit_rate: f64,
    pub miss_rate: f64,
    pub avg_latency_ns: u64,
    pub memory_usage_bytes: u64,
    pub operations_per_second: f64,
}

/// Pattern analysis result (from types.rs - ML analytics)
#[derive(Debug, Clone)]
pub struct PatternAnalysis {
    pub access_frequency: f64,
    pub temporal_locality: f64,
    pub spatial_locality: f64,
    pub prediction_confidence: f64,
}

/// Policy decision result (from types.rs - policy engine integration)
#[derive(Debug, Clone)]
pub struct PolicyDecision {
    pub action: PolicyAction,
    pub confidence: f64,
    pub reasoning: String,
}
