//! Error types for warm tier initialization
//!
//! This module defines error types for safe warm tier construction,
//! replacing panic-prone expect() calls with proper error handling.

use std::fmt;
use std::time::Duration;

use crate::cache::traits::types_and_enums::CacheOperationError;

/// Error type for warm tier initialization failures
#[allow(dead_code)] // Warm tier error - comprehensive initialization error enumeration
#[derive(Debug)]
pub enum WarmTierInitError {
    MemoryMonitorCreation(CacheOperationError),
    MemoryAllocationExhausted { attempts: usize },
    SystemMemoryDetection { reason: String },
    ConfigurationError(String),
    InitializationTimeout { duration_ms: u64 },
}

impl fmt::Display for WarmTierInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WarmTierInitError::MemoryMonitorCreation(err) => {
                write!(f, "Memory monitor creation failed: {}", err)
            }
            WarmTierInitError::MemoryAllocationExhausted { attempts } => {
                write!(f, "Memory allocation failed after {} attempts", attempts)
            }
            WarmTierInitError::SystemMemoryDetection { reason } => {
                write!(f, "System memory detection failed: {}", reason)
            }
            WarmTierInitError::ConfigurationError(msg) => {
                write!(f, "Configuration validation failed: {}", msg)
            }
            WarmTierInitError::InitializationTimeout { duration_ms } => {
                write!(f, "Resource initialization timeout after {}ms", duration_ms)
            }
        }
    }
}

impl std::error::Error for WarmTierInitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WarmTierInitError::MemoryMonitorCreation(err) => Some(err),
            _ => None,
        }
    }
}

impl From<CacheOperationError> for WarmTierInitError {
    fn from(err: CacheOperationError) -> Self {
        WarmTierInitError::MemoryMonitorCreation(err)
    }
}

#[allow(dead_code)] // Warm tier error - error handling methods for retry logic
impl WarmTierInitError {
    /// Check if this error might be resolved by retrying
    pub fn is_retryable(&self) -> bool {
        match self {
            // Memory monitor creation might be retryable if it's due to temporary resource exhaustion
            WarmTierInitError::MemoryMonitorCreation(cache_err) => {
                // Check if the underlying error suggests retryable conditions
                cache_err.to_string().contains("resource")
                    || cache_err.to_string().contains("temporary")
                    || cache_err.to_string().contains("timeout")
            }
            // System memory detection might be retryable if it's due to temporary system issues
            WarmTierInitError::SystemMemoryDetection { reason } => {
                reason.contains("temporary")
                    || reason.contains("timeout")
                    || reason.contains("busy")
            }
            // Configuration errors are typically not retryable
            WarmTierInitError::ConfigurationError(_) => false,
            // Timeout errors might be retryable with a longer timeout
            WarmTierInitError::InitializationTimeout { .. } => true,
            // Memory allocation exhausted is not typically retryable immediately
            WarmTierInitError::MemoryAllocationExhausted { .. } => false,
        }
    }
}

/// Retry configuration for warm tier initialization
#[allow(dead_code)] // Warm tier error - retry configuration for initialization backoff
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

#[allow(dead_code)] // Warm tier error - retry config methods for backoff calculation
impl RetryConfig {
    /// Calculate exponential backoff delay
    pub fn calculate_backoff(&self, attempt: usize) -> Duration {
        let delay_ms =
            (self.base_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32 - 1)) as u64;
        let capped_delay = delay_ms.min(self.max_delay_ms);
        Duration::from_millis(capped_delay)
    }
}

/// Degradation modes for warm tier initialization
#[allow(dead_code)] // Warm tier error - degradation mode enumeration for fallback strategies
#[derive(Debug, Clone, PartialEq)]
pub enum DegradationMode {
    /// Fail immediately on initialization errors
    ErrorOnFailure,
    /// Fall back to memory monitor-less mode
    MonitorlessMode,
}
