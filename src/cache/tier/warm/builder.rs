#![allow(dead_code)]
// Warm tier builder - Complete builder pattern library with safe initialization, retry logic, graceful degradation, and error handling

//! Builder pattern for safe warm tier initialization
//!
//! This module implements a builder pattern for WarmTier construction with
//! retry logic, graceful degradation, and proper error handling.

use super::config::WarmTierConfig;
use super::core::LockFreeWarmTier;
use super::error::{DegradationMode, RetryConfig, WarmTierInitError};

/// Builder for warm tier cache with retry logic and graceful degradation
pub struct WarmTierBuilder {
    config: WarmTierConfig,
    retry_config: RetryConfig,
    degradation_mode: DegradationMode,
}

impl WarmTierBuilder {
    /// Create new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: WarmTierConfig::default(),
            retry_config: RetryConfig::default(),
            degradation_mode: DegradationMode::ErrorOnFailure,
        }
    }

    /// Configure memory limit
    pub fn memory_limit(mut self, limit: u64) -> Self {
        self.config.max_memory_bytes = limit;
        self
    }

    /// Configure maximum entries
    pub fn max_entries(mut self, max_entries: usize) -> Self {
        self.config.max_entries = max_entries;
        self
    }

    /// Configure retry settings
    pub fn retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// Enable graceful degradation mode
    pub fn graceful_degradation(mut self) -> Self {
        self.degradation_mode = DegradationMode::MonitorlessMode;
        self
    }

    /// Set degradation mode explicitly
    pub fn degradation_mode(mut self, mode: DegradationMode) -> Self {
        self.degradation_mode = mode;
        self
    }

    /// Build warm tier with retry logic
    pub fn build<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue + Default,
    >(
        self,
    ) -> Result<LockFreeWarmTier<K, V>, WarmTierInitError> {
        let mut attempts = 0;
        let max_attempts = self.retry_config.max_attempts;

        loop {
            match LockFreeWarmTier::new(self.config.clone()) {
                Ok(tier) => return Ok(tier),
                Err(e) if attempts < max_attempts => {
                    // Only retry if the error is retryable
                    if e.is_retryable() {
                        attempts += 1;
                        let backoff = self.retry_config.calculate_backoff(attempts);
                        std::thread::sleep(backoff);
                        continue;
                    } else {
                        // Non-retryable error, proceed to degradation logic immediately
                        return match self.degradation_mode {
                            DegradationMode::MonitorlessMode => LockFreeWarmTier::new(self.config),
                            DegradationMode::ErrorOnFailure => Err(e),
                        };
                    }
                }
                Err(e) => {
                    // Exhausted retries, try graceful degradation
                    return match self.degradation_mode {
                        DegradationMode::MonitorlessMode => LockFreeWarmTier::new(self.config),
                        DegradationMode::ErrorOnFailure => Err(e),
                    };
                }
            }
        }
    }
}

impl Default for WarmTierBuilder {
    fn default() -> Self {
        Self::new()
    }
}
