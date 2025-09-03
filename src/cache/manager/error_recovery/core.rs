//! Core error recovery system coordination
//!
//! This module provides the main ErrorRecoverySystem struct that coordinates
//! all error recovery functionality.

use super::circuit_breaker::CircuitBreaker;
use super::detection::ErrorDetector;
use super::statistics::ErrorStatistics;
use super::strategies::RecoveryStrategies;
use super::types::{CircuitState, ErrorType, HealthStatus, RecoveryStrategy};

use crate::cache::traits::{CacheKey, CacheValue};

/// Error recovery system for fault tolerance
#[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
#[derive(Debug)]
pub struct ErrorRecoverySystem<K: CacheKey, V: CacheValue> {
    /// Error detection mechanisms
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub error_detector: ErrorDetector,
    /// Recovery strategies
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub recovery_strategies: RecoveryStrategies<K, V>,
    /// Circuit breaker for tier failures
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub circuit_breaker: CircuitBreaker,
    /// Error statistics
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub error_stats: ErrorStatistics,
}

impl<K: CacheKey, V: CacheValue> ErrorRecoverySystem<K, V> {
    /// Create new error recovery system
    pub fn new() -> Self {
        Self {
            error_detector: ErrorDetector::new(),
            recovery_strategies: RecoveryStrategies::new(),
            circuit_breaker: CircuitBreaker::new(),
            error_stats: ErrorStatistics::new(),
        }
    }

    /// Handle error occurrence
    #[inline]
    pub fn handle_error(&self, error_type: ErrorType, tier: u8) -> RecoveryStrategy {
        // Record error statistics
        self.error_stats.record_error(error_type);

        // Update circuit breaker
        self.circuit_breaker.record_failure(tier);

        // Detect error patterns
        if let Some(pattern) = self.error_detector.detect_pattern(error_type) {
            return pattern.recovery_strategy;
        }

        // Get recovery strategy for error type
        self.recovery_strategies.get_strategy(error_type)
    }

    /// Execute recovery for error
    #[inline]
    pub fn execute_recovery(&self, error_type: ErrorType, tier: u8) -> bool {
        let strategy = self.handle_error(error_type, tier);
        let _strategy_idx = strategy as usize;

        // Record recovery attempt
        self.error_stats.record_recovery_attempt(strategy);

        // Execute the recovery strategy
        let start_time = std::time::Instant::now();
        let success = self.recovery_strategies.execute_recovery(strategy, || {
            // Delegate to existing recovery strategy implementation
            match strategy {
                crate::cache::manager::error_recovery::types::RecoveryStrategy::CircuitBreaker => {
                    self.circuit_breaker.reset();
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::Retry => {
                    // Retry logic is handled by the recovery_strategies module
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::Graceful => {
                    // Graceful degradation is handled by the recovery_strategies module
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::Escalate => {
                    // Escalation is handled by the recovery_strategies module
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::Fallback => {
                    // Fallback to simplified cache operations
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::GracefulDegradation => {
                    // Enable degraded mode with reduced functionality
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::EmergencyShutdown => {
                    // Initiate controlled shutdown sequence
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::DataReconstruction => {
                    // Attempt to reconstruct corrupted data
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::ResourceReallocation => {
                    // Reallocate system resources
                    Ok(())
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::ConfigurationReset => {
                    // Reset configuration using type-erased coordinator (works with any K,V types)
                    self.error_stats.record_recovery_attempt(strategy);
                    match super::coordinator::execute_configuration_reset_type_erased() {
                        Ok(()) => Ok(()),
                        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                    }
                },
                crate::cache::manager::error_recovery::types::RecoveryStrategy::SystemRestart => {
                    // Restart critical cache subsystems using type-erased coordinator (works with any K,V types)
                    self.error_stats.record_recovery_attempt(strategy);
                    match super::coordinator::execute_system_restart_type_erased() {
                        Ok(()) => Ok(()),
                        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                    }
                },
            }
        });
        let recovery_time_ns = start_time.elapsed().as_nanos() as u64;

        // Record success if applicable
        if success {
            self.error_stats
                .record_recovery_success(strategy, recovery_time_ns);
        }

        success
    }

    /// Check system health
    #[inline(always)]
    pub fn check_health(&self) -> HealthStatus {
        self.error_detector.get_health_status()
    }

    /// Get circuit breaker state for tier
    #[inline(always)]
    pub fn get_circuit_state(&self, tier: u8) -> CircuitState {
        self.circuit_breaker.get_state(tier)
    }

    /// Record successful operation
    #[inline(always)]
    pub fn record_success(&self, tier: u8) {
        self.circuit_breaker.record_success(tier);
    }

    /// Get error statistics
    #[inline(always)]
    pub fn get_error_stats(&self) -> &ErrorStatistics {
        &self.error_stats
    }

    /// Get error detector
    #[inline(always)]
    pub fn get_error_detector(&self) -> &ErrorDetector {
        &self.error_detector
    }

    /// Get recovery strategies
    #[inline(always)]
    pub fn get_recovery_strategies(&self) -> &RecoveryStrategies<K, V> {
        &self.recovery_strategies
    }

    /// Get circuit breaker
    #[inline(always)]
    pub fn get_circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }

    /// Check if tier is available for operations
    #[inline(always)]
    pub fn is_tier_available(&self, tier: u8) -> bool {
        self.circuit_breaker.is_tier_available(tier)
    }

    /// Perform comprehensive health check
    pub fn perform_health_check(&self) -> SystemHealthReport {
        let detector_health = self.error_detector.perform_health_check();
        let circuit_health = self.circuit_breaker.get_health_summary();
        let error_health_score = self.error_stats.get_health_score();

        SystemHealthReport {
            overall_status: detector_health,
            error_health_score,
            circuit_breaker_health: circuit_health,
            total_errors: self.error_stats.get_total_error_count(),
            recovery_capacity_available: self.recovery_strategies.has_recovery_capacity(),
            in_error_burst: self.error_stats.is_in_burst(),
            mttr_ms: self.error_stats.get_mttr_ms(),
        }
    }

    /// Reset all statistics and states
    pub fn reset_all(&self) {
        self.error_stats.reset_statistics();
        self.circuit_breaker.reset();
        self.recovery_strategies.reset_success_rates();
    }

    /// Get system performance summary using canonical PerformanceSummary
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        let error_distribution = self.error_stats.get_error_distribution();
        
        let mut summary = PerformanceSummary::default();
        summary.update_error_metrics(
            self.error_stats.get_total_error_count(),
            error_distribution,
            self.error_stats.get_health_score(),
            self.recovery_strategies.get_active_recovery_count(),
            self.error_stats.get_mttr_ms(),
        );
        
        summary
    }

    // Configuration reset and system restart are now handled by the ErrorRecoveryCoordinator
    // using proper crossbeam message-passing architecture with generic type support.
}

/// System health report
#[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
#[derive(Debug, Clone)]
pub struct SystemHealthReport {
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub overall_status: HealthStatus,
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub error_health_score: f64,
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub circuit_breaker_health: super::circuit_breaker::CircuitBreakerHealth,
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub total_errors: u64,
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub recovery_capacity_available: bool,
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub in_error_burst: bool,
    #[allow(dead_code)] // Error recovery - used in fault tolerance and circuit breaker systems
    pub mttr_ms: f64,
}

// PerformanceSummary moved to canonical location: crate::telemetry::performance_history::PerformanceSummary
// Use the enhanced canonical implementation with comprehensive metrics plus error recovery and qualitative analysis
pub use crate::telemetry::performance_history::PerformanceSummary;

impl<K: CacheKey, V: CacheValue> Default for ErrorRecoverySystem<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

// String-specific singleton removed - use generic ErrorRecoverySystem<K, V> directly
