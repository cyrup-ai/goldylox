//! Error detection mechanisms for fault tolerance
//!
//! Provides error pattern recognition, health checking, and detection
//! state management for the error recovery system.

use std::time::Instant;
use std::sync::atomic::Ordering;

use super::types::*;
use super::circuit_breaker::{CircuitBreaker, CircuitState};
use super::statistics::ErrorStatistics;

/// Error detection mechanisms
#[derive(Debug)]
pub struct ErrorDetector {
    /// Error rate thresholds
    error_thresholds: ErrorThresholds,
    /// Error pattern recognition
    pattern_detector: ErrorPatternDetector,
    /// Health check configuration
    health_check_config: HealthCheckConfig,
    /// Detection state
    detection_state: DetectionState,
}

impl ErrorDetector {
    /// Create new error detector
    pub fn new() -> Self {
        Self {
            error_thresholds: ErrorThresholds::default(),
            pattern_detector: ErrorPatternDetector::new(),
            health_check_config: HealthCheckConfig::default(),
            detection_state: DetectionState::new(),
        }
    }

    /// Detect error patterns
    #[inline]
    pub fn detect_pattern(&self, error_type: ErrorType) -> Option<&ErrorPattern> {
        self.pattern_detector.match_pattern(error_type)
    }

    /// Get current health status
    #[inline(always)]
    pub fn get_health_status(&self) -> HealthStatus {
        self.detection_state.health_status.load()
    }

    /// Perform health check
    #[inline]
    pub fn perform_health_check(
        &self,
        circuit_breaker: &CircuitBreaker,
        error_stats: &ErrorStatistics,
    ) -> HealthStatus {
        let now = Instant::now();
        let last_check = self.detection_state.last_health_check.load();

        if now.duration_since(last_check) >= self.health_check_config.check_interval {
            let status = self.evaluate_system_health(circuit_breaker, error_stats);
            self.detection_state.health_status.store(status);
            self.detection_state.last_health_check.store(now);
            status
        } else {
            self.detection_state.health_status.load()
        }
    }

    /// Evaluate system health based on current metrics
    fn evaluate_system_health(
        &self,
        circuit_breaker: &CircuitBreaker,
        error_stats: &ErrorStatistics,
    ) -> HealthStatus {
        // Get circuit breaker health summary
        let circuit_health = circuit_breaker.get_health_summary();
        
        // Get error statistics
        let total_errors = error_stats.get_total_error_count();
        let health_score = error_stats.get_health_score();
        let in_burst = error_stats.is_in_burst();
        
        // Get configured thresholds
        let max_error_rate = self.error_thresholds.max_error_rate.load(Ordering::Relaxed);
        let burst_threshold = self.error_thresholds.burst_threshold.load(Ordering::Relaxed);
        let critical_threshold = self.error_thresholds.critical_threshold.load(Ordering::Relaxed);
        
        // Check circuit breaker states
        let open_circuits = [
            circuit_health.hot_tier_state,
            circuit_health.warm_tier_state,
            circuit_health.cold_tier_state,
        ]
        .iter()
        .filter(|s| **s == CircuitState::Open)
        .count();
        
        // Calculate failure ratio
        let total_ops = circuit_health.total_failures as u64 + circuit_health.total_successes as u64;
        let failure_ratio = if total_ops > 0 {
            circuit_health.total_failures as f64 / total_ops as f64
        } else {
            0.0
        };
        
        // Calculate error rate (errors per recent time window)
        // Using total_errors as a proxy for rate since we don't have time windows
        let error_rate_exceeded = total_errors > max_error_rate as u64;
        let burst_detected = in_burst || total_errors > burst_threshold as u64;
        let critical_errors = total_errors > critical_threshold as u64;
        
        // Determine health status based on multiple factors
        // HealthStatus values are: Healthy, Degraded, Critical, Failed
        if critical_errors || failure_ratio > 0.5 || open_circuits >= 2 {
            HealthStatus::Failed
        } else if error_rate_exceeded || failure_ratio > 0.2 || burst_detected || open_circuits >= 1 {
            HealthStatus::Critical
        } else if health_score < 0.7 || failure_ratio > 0.1 || total_errors > (max_error_rate / 2) as u64 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Update detection sensitivity
    #[inline]
    pub fn set_sensitivity(&self, sensitivity: f32) {
        self.detection_state.sensitivity.store(sensitivity);
    }

    /// Get current detection sensitivity
    #[inline(always)]
    pub fn get_sensitivity(&self) -> f32 {
        self.detection_state.sensitivity.load()
    }


}
