//! Cache operation results and error handling
//!
//! This module provides comprehensive error handling and result types
//! for cache operations with detailed metadata and recovery hints.

use crate::cache::traits::*;
pub use crate::cache::types::error_types::HitStatus; // Canonical location

/// Cache operation result with rich metadata
#[allow(dead_code)] // Cache types - comprehensive cache result structure with metadata
#[derive(Debug, Clone)]
pub struct CacheResult<V: CacheValue> {
    /// Operation success status
    pub success: bool,
    /// Retrieved value (if successful)
    pub value: Option<V>,
    /// Operation latency in nanoseconds
    pub latency_ns: u64,
    /// Tier that served the request
    pub tier: TierLocation,
    /// Cache hit/miss status
    pub hit_status: HitStatus,
    /// Additional operation metadata
    pub metadata: OperationMetadata,
}

#[allow(dead_code)] // Cache types - cache result methods for comprehensive operation tracking
impl<V: CacheValue> CacheResult<V> {
    /// Create successful cache hit result
    #[inline(always)]
    pub fn hit(value: V, latency_ns: u64, tier: TierLocation) -> Self {
        Self {
            success: true,
            value: Some(value),
            latency_ns,
            tier,
            hit_status: HitStatus::Hit,
            metadata: OperationMetadata::default(),
        }
    }

    /// Create cache miss result
    #[inline(always)]
    pub fn miss(latency_ns: u64) -> Self {
        Self {
            success: false,
            value: None,
            latency_ns,
            tier: TierLocation::Hot, // Start search from hot tier
            hit_status: HitStatus::Miss,
            metadata: OperationMetadata::default(),
        }
    }

    /// Create error result
    #[inline(always)]
    pub fn error(error: impl Into<CacheOperationError>, latency_ns: u64) -> Self {
        Self {
            success: false,
            value: None,
            latency_ns,
            tier: TierLocation::Hot,
            hit_status: HitStatus::Error,
            metadata: OperationMetadata {
                error: Some(error.into()),
                ..Default::default()
            },
        }
    }

    /// Check if operation was successful
    #[inline(always)]
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Check if this was a cache hit
    #[inline(always)]
    pub fn is_hit(&self) -> bool {
        matches!(self.hit_status, HitStatus::Hit)
    }
}

/// Cache operation metadata for observability
#[allow(dead_code)] // Cache types - operation metadata structure for comprehensive observability
#[derive(Debug, Clone, Default)]
pub struct OperationMetadata {
    /// Operation timestamp
    pub timestamp_ns: u64,
    /// Number of tiers searched
    pub tiers_searched: u8,
    /// Memory allocation count (should be 0)
    pub allocations: u16,
    /// SIMD operations performed
    pub simd_ops: u16,
    /// Error information (if any)
    pub error: Option<CacheOperationError>,
}

// HitStatus CANONICALIZED: moved to canonical location:
// crate::cache::types::error_types::HitStatus
// Use the canonical enum version with comprehensive multi-tier support

// CacheOperationError moved to canonical location: crate::cache::traits::types_and_enums::CacheOperationError
// Use the enhanced enum version with recovery hints, error codes, and production-quality features.

/// Generic cache processing errors
#[derive(Debug, Clone)]
pub struct CacheError {
    /// Error message
    pub message: String,
    /// Error code
    pub code: u32,
    /// Timestamp when error occurred
    pub timestamp: std::time::SystemTime,
    /// Additional context about the error
    pub context: String,
}

impl CacheError {
    /// Create corruption error
    pub fn corruption(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7001,
            timestamp: std::time::SystemTime::now(),
            context: "Data corruption detected".to_string(),
        }
    }

    /// Create processing error
    pub fn processing_failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7002,
            timestamp: std::time::SystemTime::now(),
            context: "Processing operation failed".to_string(),
        }
    }

    /// Create not found error
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7003,
            timestamp: std::time::SystemTime::now(),
            context: "Resource not found".to_string(),
        }
    }

    /// Create invalid operation error
    pub fn invalid_operation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7004,
            timestamp: std::time::SystemTime::now(),
            context: "Invalid operation attempted".to_string(),
        }
    }

    /// Create internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7005,
            timestamp: std::time::SystemTime::now(),
            context: "Internal system error".to_string(),
        }
    }

    /// Create initialization failed error
    pub fn initialization_failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7006,
            timestamp: std::time::SystemTime::now(),
            context: "Initialization failed".to_string(),
        }
    }

    /// Create operation failed error
    pub fn operation_failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7007,
            timestamp: std::time::SystemTime::now(),
            context: "Operation failed".to_string(),
        }
    }

    /// Create serialization failed error
    pub fn serialization_failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7008,
            timestamp: std::time::SystemTime::now(),
            context: "Serialization failed".to_string(),
        }
    }

    /// Create resource exhausted error
    pub fn resource_exhausted(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7009,
            timestamp: std::time::SystemTime::now(),
            context: "Resource exhausted".to_string(),
        }
    }

    /// Create timing error
    pub fn timing_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7010,
            timestamp: std::time::SystemTime::now(),
            context: "Timing error".to_string(),
        }
    }

    /// Create configuration error
    pub fn configuration_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7011,
            timestamp: std::time::SystemTime::now(),
            context: "Configuration error".to_string(),
        }
    }

    /// Create IO failed error
    pub fn io_failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7012,
            timestamp: std::time::SystemTime::now(),
            context: "IO operation failed".to_string(),
        }
    }

    /// Create concurrency error
    pub fn concurrency_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7013,
            timestamp: std::time::SystemTime::now(),
            context: "Concurrency error".to_string(),
        }
    }

    /// Create invalid state error
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 7014,
            timestamp: std::time::SystemTime::now(),
            context: "Invalid state".to_string(),
        }
    }
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CacheError({}): {}", self.code, self.message)
    }
}

impl std::error::Error for CacheError {}

impl From<CacheError> for CacheOperationError {
    fn from(err: CacheError) -> Self {
        // Convert to appropriate enum variant based on error code
        match err.code {
            1001..=1099 => {
                CacheOperationError::SerializationError(format!("Cache error: {}", err.message))
            }
            2001..=2099 => {
                CacheOperationError::StorageError(format!("Cache error: {}", err.message))
            }
            3001..=3099 => CacheOperationError::MemoryLimitExceeded,
            _ => CacheOperationError::InternalError,
        }
    }
}

// BatchResult moved to canonical location: crate::cache::types::batch_operations::BatchResult
// Use the enhanced canonical implementation with HashMap-based indexing for O(1) key lookup,
// TimedResult wrapper for precise per-operation timing, comprehensive batch analysis methods,
// and production-ready timing infrastructure that supports parallel execution metrics

impl From<crate::cache::traits::types_and_enums::CacheOperationError> for CacheError {
    fn from(error: crate::cache::traits::types_and_enums::CacheOperationError) -> Self {
        // CacheOperationError is now canonical in types_and_enums - no need for import here
        match error {
            CacheOperationError::KeyNotFound => CacheError::not_found("Key not found"),
            CacheOperationError::SerializationError(msg) => CacheError::serialization_failed(msg),
            CacheOperationError::DeserializationError(msg) => CacheError::serialization_failed(msg),
            CacheOperationError::StorageError(msg) => CacheError::operation_failed(msg),
            CacheOperationError::MemoryLimitExceeded => {
                CacheError::resource_exhausted("Memory limit exceeded")
            }
            CacheOperationError::TimeoutError => CacheError::timing_error("Operation timed out"),
            CacheOperationError::InvalidConfiguration(msg) => CacheError::configuration_error(msg),
            CacheOperationError::TierError(msg) => CacheError::operation_failed(msg),
            CacheOperationError::Io(msg) => CacheError::io_failed(msg),
            CacheOperationError::CoherenceError => {
                CacheError::concurrency_error("Cache coherence error")
            }
            CacheOperationError::InvalidState(msg) => CacheError::invalid_state(msg),
            CacheOperationError::TierOperationFailed => {
                CacheError::operation_failed("Tier operation failed")
            }
            CacheOperationError::ResourceExhausted(msg) => CacheError::resource_exhausted(msg),
            CacheOperationError::ConfigurationError(msg) => CacheError::configuration_error(msg),
            CacheOperationError::Corruption(msg) => CacheError::corruption(msg),
            CacheOperationError::NotFound => CacheError::not_found("Not found"),
            CacheOperationError::InvalidOperation => {
                CacheError::invalid_operation("Invalid operation")
            }
            CacheOperationError::InternalError => CacheError::internal_error("Internal error"),
            CacheOperationError::InitializationFailed => {
                CacheError::initialization_failed("Initialization failed")
            }
            CacheOperationError::OperationFailed => {
                CacheError::operation_failed("Operation failed")
            }
            CacheOperationError::InvalidArgument(msg) => CacheError::invalid_operation(msg),
            CacheOperationError::ConcurrentAccess(msg) => CacheError::concurrency_error(msg),
            CacheOperationError::TimingError(msg) => CacheError::timing_error(msg),
        }
    }
}
