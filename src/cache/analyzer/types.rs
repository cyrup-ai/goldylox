//! Data structures and error types for pattern analysis
//!
//! This module provides the core data structures and error types used
//! throughout the pattern analyzer system.

// Internal analyzer architecture - components may not be used in minimal API

use crate::cache::traits::types_and_enums::CacheOperationError;

/// Analyzer-specific errors
#[derive(Debug, Clone)]
pub enum AnalyzerError {
    MemoryExhausted,
    InvalidConfiguration(String),
    TimestampError,
    ConcurrencyError,
}

impl std::fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalyzerError::MemoryExhausted => write!(f, "Pattern analyzer memory exhausted"),
            AnalyzerError::InvalidConfiguration(msg) => {
                write!(f, "Invalid analyzer configuration: {}", msg)
            }
            AnalyzerError::TimestampError => write!(f, "Timestamp calculation error"),
            AnalyzerError::ConcurrencyError => write!(f, "Concurrency error in pattern analyzer"),
        }
    }
}

impl std::error::Error for AnalyzerError {}

impl From<AnalyzerError> for CacheOperationError {
    fn from(err: AnalyzerError) -> Self {
        match err {
            AnalyzerError::MemoryExhausted => {
                CacheOperationError::resource_exhausted("Pattern analyzer memory exhausted")
            }
            AnalyzerError::InvalidConfiguration(msg) => {
                CacheOperationError::configuration_error(&msg)
            }
            AnalyzerError::TimestampError => {
                CacheOperationError::TimingError("Timestamp calculation failed".to_string())
            }
            AnalyzerError::ConcurrencyError => CacheOperationError::ConcurrentAccess(
                "Pattern analyzer concurrency error".to_string(),
            ),
        }
    }
}

/// Access pattern analysis result
#[derive(Debug, Clone)]
pub struct AccessPattern {
    /// Access frequency (accesses per second with decay)
    pub frequency: f64,
    /// Recency score (0.0 = old, 1.0 = recent)
    /// Used extensively in ML eviction decisions and policy engine
    #[allow(dead_code)]
    pub recency: f64,
    /// Temporal locality score (0.0 = no temporal pattern, 1.0 = strong temporal pattern)
    pub temporal_locality: f64,
    /// Detected pattern type
    /// Used in complex ML prediction and criteria evaluation systems
    #[allow(dead_code)]
    pub pattern_type: AccessPatternType,
}

impl Default for AccessPattern {
    fn default() -> Self {
        Self {
            frequency: 0.0,
            recency: 0.0,
            temporal_locality: 0.0,
            pattern_type: AccessPatternType::Random,
        }
    }
}

/// Types of access patterns detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPatternType {
    /// Keys accessed in sequential order
    Sequential = 0,
    /// Keys accessed randomly
    Random = 1,
    /// Keys accessed with regular time intervals
    Temporal = 2,
    /// Keys accessed with spatial locality
    Spatial = 3,
}

/// Analyzer statistics
#[derive(Debug, Clone)]
pub struct AnalyzerStatistics {
    /// Number of keys currently being tracked
    pub tracked_keys: u64,
    /// Total cleanup cycles performed
    pub total_cleanup_cycles: u64,
    /// Estimated memory usage in bytes
    pub memory_usage_bytes: u64,
}
