//! Core error recovery system coordination
//!
//! This module provides the main ErrorRecoverySystem struct that coordinates
//! all error recovery functionality.

use super::circuit_breaker::CircuitBreaker;
use super::detection::ErrorDetector;
use super::statistics::ErrorStatistics;
use super::strategies::RecoveryStrategies;
use super::types::{CircuitState, ErrorType, HealthStatus, RecoveryStrategy};

/// Error recovery system for fault tolerance
#[derive(Debug)]
pub struct ErrorRecoverySystem {
    /// Error detection mechanisms
    pub error_detector: ErrorDetector,
    /// Recovery strategies
    pub recovery_strategies: RecoveryStrategies,
    /// Circuit breaker for tier failures
    pub circuit_breaker: CircuitBreaker,
    /// Error statistics
    pub error_stats: ErrorStatistics,
}

impl ErrorRecoverySystem {
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
        let strategy_idx = strategy as usize;

        // Record recovery attempt
        self.error_stats.record_recovery_attempt(strategy_idx);

        // Execute the recovery strategy
        let start_time = std::time::Instant::now();
        let success = self.recovery_strategies.execute_recovery(strategy);
        let recovery_time_ns = start_time.elapsed().as_nanos() as u64;

        // Record success if applicable
        if success {
            self.error_stats
                .record_recovery_success(strategy_idx, recovery_time_ns);
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
    pub fn get_recovery_strategies(&self) -> &RecoveryStrategies {
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
        self.error_stats.reset_all();
        self.circuit_breaker.reset_all();
        self.recovery_strategies.reset_success_rates();
    }

    /// Get system performance summary
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        let top_errors = self.error_stats.get_top_error_types(5);
        let error_distribution = self.error_stats.get_error_distribution();

        PerformanceSummary {
            total_errors: self.error_stats.get_total_error_count(),
            top_error_types: top_errors,
            error_distribution,
            health_score: self.error_stats.get_health_score(),
            active_recoveries: self.recovery_strategies.get_active_recovery_count(),
            mttr_ms: self.error_stats.get_mttr_ms(),
        }
    }
}

/// System health report
#[derive(Debug, Clone)]
pub struct SystemHealthReport {
    pub overall_status: HealthStatus,
    pub error_health_score: f64,
    pub circuit_breaker_health: super::circuit_breaker::CircuitBreakerHealth,
    pub total_errors: u64,
    pub recovery_capacity_available: bool,
    pub in_error_burst: bool,
    pub mttr_ms: f64,
}

/// Performance summary
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub total_errors: u64,
    pub top_error_types: Vec<(ErrorType, u64)>,
    pub error_distribution: [f64; 16],
    pub health_score: f64,
    pub active_recoveries: u32,
    pub mttr_ms: f64,
}

impl Default for ErrorRecoverySystem {
    fn default() -> Self {
        Self::new()
    }
}
