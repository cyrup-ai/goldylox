//! Cache operation results and error handling types
//!
//! This module provides comprehensive error handling and result types for cache operations
//! with rich metadata and recovery hints for robust error handling.


use crate::cache::traits::*;

/// Cache operation result with rich metadata






/// Cache operation metadata for observability
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

/// Hit status enumeration for cache operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitStatus {
    /// Cache hit - value found
    Hit,
    /// Cache miss - value not found
    Miss,
    /// Operation error
    Error,
    /// Partial hit (multi-tier scenarios)
    Partial,
}

// ErrorCategory moved to canonical location: crate::cache::traits::types_and_enums

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

/// Cache operation error with recovery hints
#[derive(Debug, Clone)]
pub struct CacheOperationError {
    /// Error category
    pub category: ErrorCategory,
    /// Error message
    pub message: String,
    /// Error code for programmatic handling
    pub code: u32,
    /// Recovery suggestion
    pub recovery_hint: RecoveryHint,
    /// Whether operation can be retried
    pub retryable: bool,
}

impl CacheOperationError {
    /// Create resource exhaustion error
    #[inline(always)]
    pub fn resource_exhausted(message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Resource,
            message: message.into(),
            code: 1001,
            recovery_hint: RecoveryHint::ClearAndRetry,
            retryable: true,
        }
    }

    /// Create serialization error
    #[inline(always)]
    pub fn serialization_failed(message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Serialization,
            message: message.into(),
            code: 2001,
            recovery_hint: RecoveryHint::Fallback,
            retryable: false,
        }
    }

    /// Create IO error
    #[inline(always)]
    pub fn io_failed(message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Io,
            message: message.into(),
            code: 3001,
            recovery_hint: RecoveryHint::RetryBackoff,
            retryable: true,
        }
    }

    /// Create configuration error
    #[inline(always)]
    pub fn configuration_error(message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Configuration,
            message: message.into(),
            code: 4001,
            recovery_hint: RecoveryHint::Restart,
            retryable: false,
        }
    }

    /// Create timing error
    #[inline(always)]
    pub fn timing_error(message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Timing,
            message: message.into(),
            code: 5001,
            recovery_hint: RecoveryHint::RetryBackoff,
            retryable: true,
        }
    }

    /// Create concurrency error
    #[inline(always)]
    pub fn concurrency_error(message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Concurrency,
            message: message.into(),
            code: 6001,
            recovery_hint: RecoveryHint::RetryBackoff,
            retryable: true,
        }
    }
}
