//! Configuration for invalidation behavior
//!
//! This module defines configuration parameters and presets for invalidation
//! management including retry policies and timing parameters.

// Internal invalidation architecture - components may not be used in minimal API

/// Configuration for invalidation behavior
/// Internal invalidation configuration - fields used in invalidation management
#[derive(Debug, Clone)]
pub struct InvalidationConfig {
    /// Maximum retry attempts - used in retry logic
    #[allow(dead_code)]
    pub max_retries: u32,
    /// Retry delay in milliseconds - used in retry timing
    #[allow(dead_code)]
    pub retry_delay_ms: u64,
    /// Request timeout in milliseconds - used in timeout handling
    #[allow(dead_code)]
    pub request_timeout_ms: u64,
    /// Batch size for bulk invalidations - used in batch processing
    #[allow(dead_code)]
    pub batch_size: u32,
    /// Enable priority-based processing - used in priority handling
    #[allow(dead_code)]
    pub priority_processing: bool,
}

impl Default for InvalidationConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl InvalidationConfig {
    pub fn new() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 100,
            request_timeout_ms: 5000,
            batch_size: 32,
            priority_processing: true,
        }
    }

    #[allow(dead_code)]
    pub fn aggressive() -> Self {
        Self {
            max_retries: 1,
            retry_delay_ms: 10,
            request_timeout_ms: 1000,
            batch_size: 64,
            priority_processing: true,
        }
    }

    #[allow(dead_code)]
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
