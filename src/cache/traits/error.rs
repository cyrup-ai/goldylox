//! Cache error handling traits with comprehensive error categorization and recovery strategies
//!
//! This module provides blazing-fast error handling with zero-allocation error propagation
//! and intelligent recovery mechanisms for production cache systems.

#![allow(dead_code)] // Cache traits - Error handling trait definitions for sophisticated error management

// Internal error traits - may not be used in minimal API

use std::fmt::Debug;

use super::types_and_enums::{ErrorCategory, RecoveryHint};

/// Cache error trait with rich error information and zero-allocation propagation
pub trait CacheError: std::error::Error + Send + Sync + Debug + 'static {
    /// Error category for efficient classification
    fn category(&self) -> ErrorCategory;

    /// Retry possibility for automated recovery
    fn is_retryable(&self) -> bool;

    /// Recovery suggestions for intelligent error handling
    fn recovery_hint(&self) -> RecoveryHint;

    /// Error severity level (0-10, 10 being most severe)
    fn severity(&self) -> u8 {
        match self.category() {
            ErrorCategory::Resource => 8,
            ErrorCategory::Io => 7,
            ErrorCategory::Serialization => 5,
            ErrorCategory::Concurrency => 6,
            ErrorCategory::Configuration => 9,
            ErrorCategory::Timing => 4,
            ErrorCategory::InvalidState => 8,
        }
    }

    /// Whether error affects cache consistency
    fn affects_consistency(&self) -> bool {
        matches!(
            self.category(),
            ErrorCategory::Concurrency | ErrorCategory::Serialization
        )
    }

    /// Whether error requires immediate attention
    fn requires_immediate_attention(&self) -> bool {
        self.severity() >= 8 || self.affects_consistency()
    }

    /// Suggested backoff time in milliseconds for retryable errors
    fn backoff_time_ms(&self) -> u64 {
        if !self.is_retryable() {
            return 0;
        }

        match self.recovery_hint() {
            RecoveryHint::RetryBackoff => match self.category() {
                ErrorCategory::Resource => 1000, // 1 second for resource exhaustion
                ErrorCategory::Io => 500,        // 500ms for I/O issues
                ErrorCategory::Concurrency => 100, // 100ms for concurrency conflicts
                _ => 250,                        // 250ms default
            },
            RecoveryHint::ClearAndRetry => 2000, // 2 seconds after clearing
            _ => 0,
        }
    }

    /// Get error context for debugging (zero allocation)
    fn context(&self) -> &'static str {
        match self.category() {
            ErrorCategory::Resource => "Resource exhaustion - check memory limits",
            ErrorCategory::Io => "I/O operation failed - check storage availability",
            ErrorCategory::Serialization => "Data serialization error - check data integrity",
            ErrorCategory::Concurrency => "Concurrency conflict - retry with backoff",
            ErrorCategory::Configuration => "Configuration error - check cache settings",
            ErrorCategory::Timing => "Timing error - operation timeout or deadline exceeded",
            ErrorCategory::InvalidState => "Invalid state error - cache in inconsistent state",
        }
    }
}
