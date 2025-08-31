//! Error recovery system for fault tolerance
//!
//! This module provides comprehensive error detection, recovery strategies,
//! and circuit breaker patterns for cache fault tolerance.

pub mod circuit_breaker;
pub mod coordinator;
pub mod core;
pub mod detection;
pub mod statistics;
pub mod strategies;
pub mod types;

// Re-export main types for convenience
pub use core::{ErrorRecoverySystem, PerformanceSummary, SystemHealthReport};
pub use coordinator::{
    execute_configuration_reset, execute_system_restart, get_error_statistics,
    get_system_health, init_error_recovery_system, reset_circuit_breaker,
    shutdown_error_recovery_system,
};

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerHealth};
pub use detection::ErrorDetector;
pub use statistics::ErrorStatistics;
pub use strategies::RecoveryStrategies;
pub use types::{
    BackoffStrategy, CircuitBreakerConfig, CircuitState, CompletedPattern, DetectionState,
    ErrorBurstDetector, ErrorPattern, ErrorPatternDetector, ErrorThresholds, ErrorType,
    HealthCheckConfig, HealthStatus, PatternMatch, PatternMatchingState, PatternMatchingStats,
    RecoveryConfig, RecoveryStrategy,
};
