//! Configuration for invalidation behavior
//!
//! This module defines configuration parameters and presets for invalidation
//! management including retry policies and timing parameters.

/// Configuration for invalidation behavior
#[derive(Debug, Clone)]
pub struct InvalidationConfig {
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Batch size for bulk invalidations
    pub batch_size: u32,
    /// Enable priority-based processing
    pub priority_processing: bool,
}

impl InvalidationConfig {
    pub fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 100,
            request_timeout_ms: 5000,
            batch_size: 32,
            priority_processing: true,
        }
    }

    pub fn aggressive() -> Self {
        Self {
            max_retries: 1,
            retry_delay_ms: 10,
            request_timeout_ms: 1000,
            batch_size: 64,
            priority_processing: true,
        }
    }

    pub fn conservative() -> Self {
        Self {
            max_retries: 5,
            retry_delay_ms: 500,
            request_timeout_ms: 10000,
            batch_size: 16,
            priority_processing: false,
        }
    }
}
