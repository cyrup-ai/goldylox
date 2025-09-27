//! Cache operation results and error handling types
//!
//! This module provides comprehensive error handling and result types for cache operations
//! with rich metadata and recovery hints for robust error handling.

use crate::cache::traits::*;

/// Cache operation result with rich metadata
/// Cache operation metadata for observability
#[allow(dead_code)] // Error types - operation metadata structure for comprehensive error tracking
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
#[allow(dead_code)] // Error types - hit status enumeration with comprehensive status variants
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
#[allow(dead_code)] // Error types - recovery hint enumeration for comprehensive error recovery strategies
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

// CacheOperationError moved to canonical location: crate::cache::traits::types_and_enums::CacheOperationError
// Use the enhanced enum version with recovery hints, error codes, and production-quality features.
