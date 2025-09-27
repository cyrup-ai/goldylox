//! Type definitions, enums, and data structures for cache system
//!
//! This module contains all the supporting types, enumerations, and data structures
//! that provide configuration and metadata for the cache system.

#![allow(dead_code)] // Cache traits - comprehensive type system for all cache variants
// Internal types and enums - many may not be used in minimal API

use std::time::{Duration, Instant};

use super::supporting_types::CompressionAlgorithm;

/// Tier affinity hints for intelligent placement
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
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
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
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
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
pub enum TierLocation {
    Hot,
    Warm,
    Cold,
}

/// Access type classification for pattern recognition and operations
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
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
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
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
    /// Adaptive Replacement Cache algorithm selection
    AdaptiveReplacementCache,
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

// RecoveryHint definition moved below with CacheOperationError for better organization

/// Recovery hint for cache operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryHint {
    /// Clear cache and retry operation
    ClearAndRetry,
    /// Retry with exponential backoff
    RetryBackoff,
    /// Fall back to alternative strategy
    Fallback,
    /// Restart cache subsystem
    Restart,
    /// No recovery possible
    Fatal,
}

/// Cache operation error types - Enhanced canonical version with recovery hints
///
/// This enum combines the simplicity of pattern matching with rich error metadata
/// for production-quality error handling and recovery strategies.
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
    /// Create invalid state error
    #[inline(always)]
    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::InvalidState(msg.into())
    }

    /// Create resource exhaustion error
    #[inline(always)]
    pub fn resource_exhausted(msg: impl Into<String>) -> Self {
        Self::ResourceExhausted(msg.into())
    }

    /// Create serialization error
    #[inline(always)]
    pub fn serialization_failed(msg: impl Into<String>) -> Self {
        Self::SerializationError(msg.into())
    }

    /// Create IO error
    #[inline(always)]
    pub fn io_failed(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }

    /// Create configuration error  
    #[inline(always)]
    pub fn configuration_error(msg: impl Into<String>) -> Self {
        Self::ConfigurationError(msg.into())
    }

    /// Create timing error
    #[inline(always)]
    pub fn timing_error(msg: impl Into<String>) -> Self {
        Self::TimingError(msg.into())
    }

    /// Create concurrency error
    #[inline(always)]
    pub fn concurrency_error(msg: impl Into<String>) -> Self {
        Self::ConcurrentAccess(msg.into())
    }

    /// Get recovery hint for this error
    pub fn recovery_hint(&self) -> RecoveryHint {
        match self {
            Self::KeyNotFound | Self::NotFound => RecoveryHint::Fallback,
            Self::SerializationError(_) | Self::DeserializationError(_) => RecoveryHint::Fallback,
            Self::StorageError(_) | Self::Io(_) => RecoveryHint::RetryBackoff,
            Self::MemoryLimitExceeded | Self::ResourceExhausted(_) => RecoveryHint::ClearAndRetry,
            Self::TimeoutError | Self::TimingError(_) => RecoveryHint::RetryBackoff,
            Self::InvalidConfiguration(_) | Self::ConfigurationError(_) => RecoveryHint::Restart,
            Self::TierError(_) | Self::TierOperationFailed => RecoveryHint::RetryBackoff,
            Self::CoherenceError => RecoveryHint::ClearAndRetry,
            Self::InvalidState(_) => RecoveryHint::Restart,
            Self::Corruption(_) => RecoveryHint::Fatal,
            Self::InvalidOperation | Self::InvalidArgument(_) => RecoveryHint::Fallback,
            Self::InternalError | Self::InitializationFailed | Self::OperationFailed => {
                RecoveryHint::Restart
            }
            Self::ConcurrentAccess(_) => RecoveryHint::RetryBackoff,
        }
    }

    /// Check if operation can be retried
    pub fn retryable(&self) -> bool {
        matches!(
            self.recovery_hint(),
            RecoveryHint::ClearAndRetry | RecoveryHint::RetryBackoff | RecoveryHint::Fallback
        )
    }

    /// Get error code for programmatic handling
    pub fn code(&self) -> u32 {
        match self {
            Self::KeyNotFound => 1000,
            Self::SerializationError(_) => 2001,
            Self::DeserializationError(_) => 2002,
            Self::StorageError(_) => 3001,
            Self::MemoryLimitExceeded => 1001,
            Self::TimeoutError => 5001,
            Self::InvalidConfiguration(_) => 4001,
            Self::TierError(_) => 6001,
            Self::Io(_) => 3001,
            Self::CoherenceError => 6002,
            Self::TierOperationFailed => 6003,
            Self::InvalidState(_) => 2001,
            Self::ResourceExhausted(_) => 1001,
            Self::ConfigurationError(_) => 4001,
            Self::Corruption(_) => 7001,
            Self::NotFound => 1000,
            Self::InvalidOperation => 2003,
            Self::InternalError => 8001,
            Self::InitializationFailed => 8002,
            Self::OperationFailed => 8003,
            Self::InvalidArgument(_) => 2004,
            Self::ConcurrentAccess(_) => 6001,
            Self::TimingError(_) => 5001,
        }
    }

    pub fn initialization_failed(msg: impl Into<String>) -> Self {
        Self::InvalidConfiguration(msg.into())
    }

    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::InvalidState(msg.into())
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl CapacityInfo {
    /// Create new capacity info
    #[inline(always)]
    pub const fn new(
        current: usize,
        maximum: usize,
        memory_bytes: usize,
        memory_limit: usize,
    ) -> Self {
        Self {
            current,
            maximum,
            memory_bytes,
            memory_limit,
        }
    }

    /// Get utilization percentage (0.0-1.0)
    #[inline(always)]
    pub fn utilization(&self) -> f64 {
        if self.maximum > 0 {
            self.current as f64 / self.maximum as f64
        } else {
            0.0
        }
    }

    /// Get memory utilization percentage (0.0-1.0)
    #[inline(always)]
    pub fn memory_utilization(&self) -> f64 {
        if self.memory_limit > 0 {
            self.memory_bytes as f64 / self.memory_limit as f64
        } else {
            0.0
        }
    }

    /// Check if at capacity
    #[inline(always)]
    pub fn is_at_capacity(&self) -> bool {
        self.current >= self.maximum || self.memory_bytes >= self.memory_limit
    }

    /// Check if under memory pressure
    #[inline(always)]
    pub fn is_under_pressure(&self) -> bool {
        self.memory_utilization() > 0.8 || self.utilization() > 0.9
    }

    /// Get available capacity
    #[inline(always)]
    pub fn available_capacity(&self) -> usize {
        self.maximum.saturating_sub(self.current)
    }

    /// Get available memory
    #[inline(always)]
    pub fn available_memory(&self) -> usize {
        self.memory_limit.saturating_sub(self.memory_bytes)
    }
}

/// Maintenance report structure
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl MaintenanceReport {
    /// Create new maintenance report
    #[inline(always)]
    pub const fn new(
        cleaned_entries: usize,
        bytes_reclaimed: usize,
        duration: Duration,
        issues_resolved: usize,
    ) -> Self {
        Self {
            cleaned_entries,
            bytes_reclaimed,
            duration,
            issues_resolved,
        }
    }

    /// Check if maintenance was effective
    #[inline(always)]
    pub fn was_effective(&self) -> bool {
        self.cleaned_entries > 0 || self.bytes_reclaimed > 0 || self.issues_resolved > 0
    }

    /// Get cleanup rate (entries per second)
    #[inline(always)]
    pub fn cleanup_rate(&self) -> f64 {
        let duration_secs = self.duration.as_secs_f64();
        if duration_secs > 0.0 {
            self.cleaned_entries as f64 / duration_secs
        } else {
            0.0
        }
    }

    /// Get data reclaim rate (bytes per second)
    #[inline(always)]
    pub fn reclaim_rate(&self) -> f64 {
        let duration_secs = self.duration.as_secs_f64();
        if duration_secs > 0.0 {
            self.bytes_reclaimed as f64 / duration_secs
        } else {
            0.0
        }
    }

    /// Get issue resolution efficiency
    #[inline(always)]
    pub fn resolution_efficiency(&self) -> f64 {
        if self.cleaned_entries > 0 {
            self.issues_resolved as f64 / self.cleaned_entries as f64
        } else {
            0.0
        }
    }
}

/// Performance metrics for policy optimization
#[derive(Debug, Clone, PartialEq)]
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

impl PerformanceMetrics {
    /// Create new performance metrics
    #[inline(always)]
    pub const fn new(
        hit_rate: f64,
        avg_access_time_ns: u64,
        ops_per_second: f64,
        memory_efficiency: f64,
    ) -> Self {
        Self {
            hit_rate,
            avg_access_time_ns,
            ops_per_second,
            memory_efficiency,
        }
    }

    /// Calculate overall performance score (0.0-1.0)
    #[inline(always)]
    pub fn performance_score(&self) -> f64 {
        let hit_rate_score = self.hit_rate;
        let latency_score = 1.0 / (1.0 + (self.avg_access_time_ns as f64 / 1_000_000.0));
        let throughput_score = (self.ops_per_second / 10000.0).min(1.0);
        let efficiency_score = self.memory_efficiency;

        (hit_rate_score * 0.3)
            + (latency_score * 0.3)
            + (throughput_score * 0.2)
            + (efficiency_score * 0.2)
    }

    /// Check if performance is optimal
    #[inline(always)]
    pub fn is_optimal(&self) -> bool {
        self.performance_score() > 0.8
    }

    /// Check if performance needs attention
    #[inline(always)]
    pub fn needs_attention(&self) -> bool {
        self.performance_score() < 0.4
    }

    /// Get latency category
    #[inline(always)]
    pub fn latency_category(&self) -> crate::cache::traits::structures::LatencyCategory {
        if self.avg_access_time_ns < 1_000 {
            crate::cache::traits::structures::LatencyCategory::UltraLow
        } else if self.avg_access_time_ns < 10_000 {
            crate::cache::traits::structures::LatencyCategory::Low
        } else if self.avg_access_time_ns < 100_000 {
            crate::cache::traits::structures::LatencyCategory::Medium
        } else {
            crate::cache::traits::structures::LatencyCategory::High
        }
    }

    /// Get throughput category
    #[inline(always)]
    pub fn throughput_category(&self) -> crate::cache::traits::structures::ThroughputCategory {
        if self.ops_per_second > 100_000.0 {
            crate::cache::traits::structures::ThroughputCategory::High
        } else if self.ops_per_second > 10_000.0 {
            crate::cache::traits::structures::ThroughputCategory::Medium
        } else {
            crate::cache::traits::structures::ThroughputCategory::Low
        }
    }
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
