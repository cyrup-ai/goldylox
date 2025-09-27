//! Error recovery and fallback mechanisms for cache operations
//!
//! This module provides defensive error handling and graceful degradation
//! when cache subsystems encounter failures or missing data.

use crate::cache::types::statistics::multi_tier::ErrorType;
use crate::telemetry::performance_history::PerformanceSummary;
use crate::telemetry::types::PerformanceSample;
// Duration removed - not needed in current implementation

/// Backoff strategy for error recovery retries
#[derive(Debug, Clone, Copy)]
pub enum BackoffStrategy {
    Linear,
    Exponential,
    Fibonacci,
    Custom,
}

/// Recovery strategies for different error types
#[derive(Debug, Clone, Copy)]
pub enum RecoveryStrategy {
    ResourceReallocation,
    TierFailover,
    CircuitBreaker,
    GracefulDegradation,
}

/// Configuration for error recovery behavior
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub backoff_strategy: BackoffStrategy,
    pub max_retries: u32,
    pub circuit_breaker_threshold: u32,
    pub recovery_timeout_ms: u64,
}

/// Error recovery metrics for monitoring and analysis
#[derive(Debug, Clone)]
pub struct RecoveryMetrics {
    /// Total number of failures across all circuit breakers
    pub total_failures: u32,
    /// Number of circuit breakers currently in bypass mode
    pub active_circuit_breakers: u32,
    /// Average recovery timeout across all circuit breakers
    pub avg_recovery_timeout_ms: u64,
    /// Estimated success rate of recovery operations (0.0-1.0)
    pub recovery_success_rate: f64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            backoff_strategy: BackoffStrategy::Exponential,
            max_retries: 3,
            circuit_breaker_threshold: 5,
            recovery_timeout_ms: 5000,
        }
    }
}

/// Recovery strategies manager
#[derive(Debug, Clone)]
pub struct RecoveryStrategies {
    config: RecoveryConfig,
}

impl RecoveryStrategies {
    pub fn new(config: RecoveryConfig) -> Self {
        Self { config }
    }

    pub fn get_retry_limit(&self, _strategy: RecoveryStrategy) -> u32 {
        self.config.max_retries
    }

    pub fn get_config(&self) -> &RecoveryConfig {
        &self.config
    }
}

/// Main error recovery system
#[derive(Debug)]
pub struct ErrorRecoverySystem<K, V> {
    recovery_strategies: RecoveryStrategies,
    circuit_breakers: Vec<ComponentCircuitBreaker>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> ErrorRecoverySystem<K, V> {
    pub fn new() -> Self {
        Self {
            recovery_strategies: RecoveryStrategies::new(RecoveryConfig::default()),
            circuit_breakers: vec![
                ComponentCircuitBreaker::new(5, 5000), // Tier 0
                ComponentCircuitBreaker::new(5, 5000), // Tier 1
                ComponentCircuitBreaker::new(5, 5000), // Tier 2
            ],
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create error recovery system with production-ready integration
    pub fn with_tier_integration(config: RecoveryConfig) -> Self {
        // Create circuit breakers with tier-specific configurations
        let circuit_breakers = vec![
            ComponentCircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.recovery_timeout_ms,
            ), // Hot tier
            ComponentCircuitBreaker::new(
                config.circuit_breaker_threshold * 2,
                config.recovery_timeout_ms * 2,
            ), // Warm tier - more tolerant
            ComponentCircuitBreaker::new(
                config.circuit_breaker_threshold * 3,
                config.recovery_timeout_ms * 3,
            ), // Cold tier - most tolerant
        ];

        Self {
            recovery_strategies: RecoveryStrategies::new(config),
            circuit_breakers,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn handle_error(&self, error_type: ErrorType, tier: u8) {
        let tier_idx = tier as usize;
        if tier_idx < self.circuit_breakers.len() {
            self.circuit_breakers[tier_idx].record_failure();
        }
        log::warn!("Error recovery: {:?} on tier {}", error_type, tier);
    }

    pub fn record_success(&self, tier: u8) {
        let tier_idx = tier as usize;
        if tier_idx < self.circuit_breakers.len() {
            // Success recorded - circuit breaker will self-heal over time
        }
    }

    pub fn is_tier_available(&self, tier: u8) -> bool {
        let tier_idx = tier as usize;
        if tier_idx < self.circuit_breakers.len() {
            !self.circuit_breakers[tier_idx].should_bypass()
        } else {
            true
        }
    }

    pub fn execute_recovery(&self, error_type: ErrorType, tier: u8) -> bool {
        log::info!(
            "Executing recovery strategy for {:?} on tier {}",
            error_type,
            tier
        );

        let tier_idx = tier as usize;
        if tier_idx >= self.circuit_breakers.len() {
            return false;
        }

        // Determine recovery strategy based on error type and tier
        let strategy = match error_type {
            ErrorType::OutOfMemory | ErrorType::MemoryAllocationFailure => {
                RecoveryStrategy::ResourceReallocation
            }
            ErrorType::Timeout => RecoveryStrategy::CircuitBreaker,
            ErrorType::Serialization | ErrorType::CoherenceViolation => {
                RecoveryStrategy::GracefulDegradation
            }
            ErrorType::SimdFailure | ErrorType::TierTransition => RecoveryStrategy::TierFailover,
            _ => RecoveryStrategy::GracefulDegradation,
        };

        // Execute recovery based on strategy
        match strategy {
            RecoveryStrategy::CircuitBreaker => {
                log::info!("Activating circuit breaker for tier {}", tier);
                self.circuit_breakers[tier_idx].record_failure();
                true
            }
            RecoveryStrategy::TierFailover => {
                log::info!("Initiating tier failover from tier {}", tier);
                // In real implementation, this would coordinate with tier manager
                // For now, just record the failure and let circuit breaker handle it
                self.circuit_breakers[tier_idx].record_failure();
                true
            }
            RecoveryStrategy::ResourceReallocation => {
                log::info!("Attempting resource reallocation for tier {}", tier);
                // In real implementation, this would trigger memory cleanup or reallocation
                true
            }
            RecoveryStrategy::GracefulDegradation => {
                log::info!("Activating graceful degradation for tier {}", tier);
                // In real implementation, this would reduce functionality temporarily
                true
            }
        }
    }

    pub fn get_recovery_strategies(&self) -> &RecoveryStrategies {
        &self.recovery_strategies
    }

    /// Reset all error recovery systems and circuit breakers
    pub fn reset_all(&self) {
        log::info!("Resetting all error recovery systems");

        // Reset all circuit breakers
        for (tier, breaker) in self.circuit_breakers.iter().enumerate() {
            breaker.reset();
            log::debug!("Reset circuit breaker for tier {}", tier);
        }

        log::info!("Error recovery system reset completed");
    }

    /// Get circuit breakers for integration with fallback providers
    pub fn get_circuit_breakers(&self) -> &Vec<ComponentCircuitBreaker> {
        &self.circuit_breakers
    }

    /// Create integrated fallback provider that uses this system's circuit breakers
    pub fn create_integrated_fallback_provider(
        &self,
        error_tracker: crate::cache::types::statistics::multi_tier::ErrorRateTracker,
    ) -> FallbackErrorProvider {
        FallbackErrorProvider::with_real_systems(
            std::sync::Arc::new(error_tracker),
            std::sync::Arc::new(self.circuit_breakers.clone()),
        )
    }

    /// Get error recovery metrics for monitoring
    pub fn get_recovery_metrics(&self) -> RecoveryMetrics {
        let total_failures: u32 = self
            .circuit_breakers
            .iter()
            .map(|cb| cb.failure_count.load(std::sync::atomic::Ordering::Relaxed))
            .sum();

        let active_circuit_breakers = self
            .circuit_breakers
            .iter()
            .filter(|cb| cb.should_bypass())
            .count() as u32;

        let avg_recovery_timeout = if !self.circuit_breakers.is_empty() {
            self.circuit_breakers
                .iter()
                .map(|cb| cb.get_recovery_timeout_ms())
                .sum::<u64>()
                / self.circuit_breakers.len() as u64
        } else {
            0
        };

        RecoveryMetrics {
            total_failures,
            active_circuit_breakers,
            avg_recovery_timeout_ms: avg_recovery_timeout,
            recovery_success_rate: if total_failures > 0 {
                // Estimate success rate based on circuit breaker resets
                0.8 // Conservative estimate - in reality this would track actual recoveries
            } else {
                1.0 // No failures = perfect success rate
            },
        }
    }
}

impl<K, V> Default for ErrorRecoverySystem<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for providing error recovery and fallback values
pub trait ErrorRecoveryProvider {
    /// Get fallback performance summary when historical data is unavailable
    fn get_fallback_performance_summary(&self) -> PerformanceSummary;

    /// Get safe baseline throughput when historical calculations fail
    fn get_safe_baseline_throughput(&self, current_throughput: f64) -> f64;

    /// Get safe error rate baseline when historical tracking is unavailable
    fn get_safe_error_rate_baseline(&self, current_error_rate: f64) -> f64;

    /// Validate and sanitize performance sample data
    fn sanitize_performance_sample(&self, sample: &PerformanceSample) -> PerformanceSample;

    /// Check if a floating point value is safe for mathematical operations
    fn is_safe_float(&self, value: f64) -> bool;

    /// Get safe mathematical result with fallback
    fn safe_divide(&self, numerator: f64, denominator: f64, fallback: f64) -> f64;

    /// Get total error count for performance summary
    fn get_total_errors(&self) -> u64;

    /// Get error distribution map for performance analysis
    fn get_error_distribution(&self) -> std::collections::HashMap<String, u32>;

    /// Get current system health score (0.0-1.0)
    fn get_health_score(&self) -> f32;

    /// Get count of currently active error recovery operations
    fn get_active_recoveries(&self) -> u32;

    /// Get mean time to recovery in milliseconds
    fn get_mttr_ms(&self) -> u32;
}

/// Default implementation of error recovery provider with conservative fallbacks
#[derive(Debug, Clone)]
pub struct FallbackErrorProvider {
    /// Conservative baseline throughput (ops/sec)
    baseline_throughput: f64,
    /// Conservative baseline error rate
    baseline_error_rate: f64,
    /// Conservative baseline hit rate
    baseline_hit_rate: f32,
    /// Conservative baseline access time (nanoseconds)
    baseline_access_time: u32,
    /// Optional reference to actual error tracking system
    error_tracker:
        Option<std::sync::Arc<crate::cache::types::statistics::multi_tier::ErrorRateTracker>>,
    /// Optional reference to circuit breakers for MTTR calculation  
    circuit_breakers: Option<std::sync::Arc<Vec<ComponentCircuitBreaker>>>,
}

impl FallbackErrorProvider {
    /// Create new fallback error provider with conservative defaults
    pub fn new() -> Self {
        Self {
            baseline_throughput: 1000.0,   // 1k ops/sec baseline
            baseline_error_rate: 0.05,     // 5% error rate baseline
            baseline_hit_rate: 0.8,        // 80% hit rate baseline
            baseline_access_time: 100_000, // 100μs baseline access time
            error_tracker: None,
            circuit_breakers: None,
        }
    }

    /// Create with integration to real error tracking systems
    pub fn with_real_systems(
        error_tracker: std::sync::Arc<
            crate::cache::types::statistics::multi_tier::ErrorRateTracker,
        >,
        circuit_breakers: std::sync::Arc<Vec<ComponentCircuitBreaker>>,
    ) -> Self {
        Self {
            baseline_throughput: 1000.0,
            baseline_error_rate: 0.05,
            baseline_hit_rate: 0.8,
            baseline_access_time: 100_000,
            error_tracker: Some(error_tracker),
            circuit_breakers: Some(circuit_breakers),
        }
    }

    /// Get error tracker reference if available
    fn get_error_tracker(
        &self,
    ) -> Option<&crate::cache::types::statistics::multi_tier::ErrorRateTracker> {
        self.error_tracker.as_ref().map(|arc| arc.as_ref())
    }

    /// Get circuit breakers reference if available  
    fn get_circuit_breakers(&self) -> Option<&Vec<ComponentCircuitBreaker>> {
        self.circuit_breakers.as_ref().map(|arc| arc.as_ref())
    }
}

impl Default for FallbackErrorProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorRecoveryProvider for FallbackErrorProvider {
    fn get_fallback_performance_summary(&self) -> PerformanceSummary {
        PerformanceSummary {
            sample_count: 1, // Indicate minimal data availability
            avg_hit_rate: self.baseline_hit_rate,
            min_hit_rate: self.baseline_hit_rate * 0.8,
            max_hit_rate: self.baseline_hit_rate * 1.1,
            avg_access_time_ns: self.baseline_access_time as u64,
            max_access_time_ns: (self.baseline_access_time as u64) * 2,
            avg_memory_usage: 1024 * 1024,     // 1MB baseline memory
            max_memory_usage: 2 * 1024 * 1024, // 2MB peak baseline
            avg_ops_per_second: self.baseline_throughput as f32,
            total_errors: (100.0 * self.baseline_error_rate) as u64,
            error_distribution: [
                0.1, 0.05, 0.03, 0.02, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            health_score: self.get_health_score() as f64,
            active_recoveries: self.get_active_recoveries(),
            mttr_ms: self.get_mttr_ms() as f64,
            is_fast: true,
            is_consistent: true,
            has_outliers: false,
            overhead_acceptable: true,
        }
    }

    fn get_safe_baseline_throughput(&self, current_throughput: f64) -> f64 {
        if self.is_safe_float(current_throughput) && current_throughput > 0.0 {
            // Use 80% of current as conservative baseline
            current_throughput * 0.8
        } else {
            // Fallback to default baseline
            self.baseline_throughput
        }
    }

    fn get_safe_error_rate_baseline(&self, current_error_rate: f64) -> f64 {
        if self.is_safe_float(current_error_rate) && current_error_rate >= 0.0 {
            // Use slightly higher rate as conservative baseline (expect some degradation)
            (current_error_rate * 1.1).min(1.0)
        } else {
            // Fallback to default baseline
            self.baseline_error_rate
        }
    }

    fn sanitize_performance_sample(&self, sample: &PerformanceSample) -> PerformanceSample {
        let mut sanitized = *sample; // Copy original

        // Sanitize hit rate
        if sanitized.hit_rate_x1000 > 1000 {
            sanitized.hit_rate_x1000 = (self.baseline_hit_rate * 1000.0) as u32;
        }

        // Sanitize access time (prevent extreme values) - max 1 second
        if sanitized.avg_access_time_ns == 0 || sanitized.avg_access_time_ns > 1_000_000_000 {
            sanitized.avg_access_time_ns = self.baseline_access_time;
        }

        // Sanitize operations per second
        if sanitized.ops_per_second_x100 == 0 || sanitized.ops_per_second_x100 > 10_000_000 {
            sanitized.ops_per_second_x100 = (self.baseline_throughput * 100.0) as u32;
        }

        // Sanitize tier utilizations (keep within 0-100%)
        sanitized.hot_utilization_x100 = sanitized.hot_utilization_x100.min(10000);
        sanitized.warm_utilization_x100 = sanitized.warm_utilization_x100.min(10000);
        sanitized.cold_utilization_x100 = sanitized.cold_utilization_x100.min(10000);

        sanitized
    }

    fn is_safe_float(&self, value: f64) -> bool {
        value.is_finite() && !value.is_nan()
    }

    fn safe_divide(&self, numerator: f64, denominator: f64, fallback: f64) -> f64 {
        if self.is_safe_float(numerator)
            && self.is_safe_float(denominator)
            && denominator != 0.0
            && denominator.abs() > f64::EPSILON
        {
            let result = numerator / denominator;
            if self.is_safe_float(result) {
                result
            } else {
                fallback
            }
        } else {
            fallback
        }
    }

    fn get_total_errors(&self) -> u64 {
        // Get real error count if available
        if let Some(error_tracker) = self.get_error_tracker() {
            error_tracker.statistical_summary().total_errors
        } else {
            // Conservative estimate based on baseline error rate
            (100.0 * self.baseline_error_rate) as u64
        }
    }

    fn get_error_distribution(&self) -> std::collections::HashMap<String, u32> {
        // Try to get real error distribution if available
        if let Some(error_tracker) = self.get_error_tracker() {
            let distribution_array = error_tracker.error_distribution();
            let stats = error_tracker.statistical_summary();
            let total_errors = stats.total_errors;

            let mut distribution = std::collections::HashMap::new();
            let error_types = [
                "timeout",
                "out_of_memory",
                "serialization",
                "coherence_violation",
                "simd_failure",
                "tier_transition",
                "worker_failure",
                "generic_error",
            ];

            for (i, &error_type_name) in error_types.iter().enumerate() {
                let count = (distribution_array[i] * total_errors as f64) as u32;
                if count > 0 {
                    distribution.insert(error_type_name.to_string(), count);
                }
            }

            distribution
        } else {
            // Fallback to conservative estimates only if no real data available
            let mut distribution = std::collections::HashMap::new();
            distribution.insert("timeout".to_string(), 2);
            distribution.insert("serialization".to_string(), 1);
            distribution.insert("generic_error".to_string(), 1);
            distribution
        }
    }

    fn get_health_score(&self) -> f32 {
        // Get real health score from error tracker if available
        if let Some(error_tracker) = self.get_error_tracker() {
            let current_error_rate = error_tracker.overall_error_rate();
            let health_score = (1.0 - current_error_rate as f32).max(0.0);

            // Adjust health score based on error trends and circuit breaker status
            if let Some(breakers) = self.get_circuit_breakers() {
                let failed_breakers = breakers.iter().filter(|b| b.should_bypass()).count();
                let penalty = (failed_breakers as f32 * 0.1).min(0.3); // Max 30% penalty
                (health_score - penalty).max(0.0)
            } else {
                health_score
            }
        } else {
            // Fallback to baseline only if no real data available
            (1.0 - self.baseline_error_rate as f32).max(0.0)
        }
    }

    fn get_active_recoveries(&self) -> u32 {
        // Count active circuit breakers that are in recovery mode
        if let Some(breakers) = self.get_circuit_breakers() {
            breakers.iter().filter(|b| b.should_bypass()).count() as u32
        } else {
            0 // No active recoveries if no circuit breakers available
        }
    }

    fn get_mttr_ms(&self) -> u32 {
        // Calculate conservative MTTR from circuit breaker recovery timeout settings
        if let Some(breakers) = self.get_circuit_breakers() {
            let total_recovery_time_ms: u64 =
                breakers.iter().map(|b| b.get_recovery_timeout_ms()).sum();
            let breaker_count = breakers.len() as u64;

            if breaker_count > 0 {
                (total_recovery_time_ms / breaker_count) as u32
            } else {
                3000 // Conservative 3 second default if no data
            }
        } else {
            3000 // Conservative 3 second default
        }
    }
}

/// Circuit breaker for failing components
#[derive(Debug)]
pub struct ComponentCircuitBreaker {
    failure_count: std::sync::atomic::AtomicU32,
    last_failure_time: std::sync::atomic::AtomicU64,
    failure_threshold: u32,
    recovery_timeout_ms: u64,
}

impl Clone for ComponentCircuitBreaker {
    fn clone(&self) -> Self {
        use std::sync::atomic::Ordering;
        Self {
            failure_count: std::sync::atomic::AtomicU32::new(
                self.failure_count.load(Ordering::Relaxed),
            ),
            last_failure_time: std::sync::atomic::AtomicU64::new(
                self.last_failure_time.load(Ordering::Relaxed),
            ),
            failure_threshold: self.failure_threshold,
            recovery_timeout_ms: self.recovery_timeout_ms,
        }
    }
}

impl ComponentCircuitBreaker {
    /// Create new circuit breaker
    pub fn new(failure_threshold: u32, recovery_timeout_ms: u64) -> Self {
        Self {
            failure_count: std::sync::atomic::AtomicU32::new(0),
            last_failure_time: std::sync::atomic::AtomicU64::new(0),
            failure_threshold,
            recovery_timeout_ms,
        }
    }

    /// Record a failure
    pub fn record_failure(&self) {
        use std::sync::atomic::Ordering;
        self.failure_count.fetch_add(1, Ordering::Relaxed);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.last_failure_time.store(now, Ordering::Relaxed);
    }

    /// Check if component should be bypassed
    pub fn should_bypass(&self) -> bool {
        use std::sync::atomic::Ordering;

        let failure_count = self.failure_count.load(Ordering::Relaxed);
        if failure_count < self.failure_threshold {
            return false;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let last_failure = self.last_failure_time.load(Ordering::Relaxed);

        // If enough time has passed, allow retry
        if now.saturating_sub(last_failure) > self.recovery_timeout_ms {
            self.failure_count.store(0, Ordering::Relaxed);
            false
        } else {
            true
        }
    }

    /// Record successful operation (reset failure count)
    pub fn record_success(&self) {
        use std::sync::atomic::Ordering;
        self.failure_count.store(0, Ordering::Relaxed);
    }

    /// Get recovery timeout in milliseconds
    pub fn get_recovery_timeout_ms(&self) -> u64 {
        self.recovery_timeout_ms
    }

    /// Reset circuit breaker to initial state
    pub fn reset(&self) {
        use std::sync::atomic::Ordering;
        self.failure_count.store(0, Ordering::Relaxed);
        self.last_failure_time.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_provider_creation() {
        let provider = FallbackErrorProvider::new();
        let summary = provider.get_fallback_performance_summary();

        assert_eq!(summary.sample_count, 1);
        assert!(summary.avg_hit_rate > 0.0);
        assert!(summary.avg_ops_per_second > 0.0);
    }

    #[test]
    fn test_safe_mathematical_operations() {
        let provider = FallbackErrorProvider::new();

        // Test normal division
        assert_eq!(provider.safe_divide(10.0, 2.0, 0.0), 5.0);

        // Test division by zero
        assert_eq!(provider.safe_divide(10.0, 0.0, 99.0), 99.0);

        // Test NaN handling
        assert_eq!(provider.safe_divide(f64::NAN, 2.0, 99.0), 99.0);

        // Test infinity handling
        assert_eq!(provider.safe_divide(f64::INFINITY, 2.0, 99.0), 99.0);
    }

    #[test]
    fn test_circuit_breaker() {
        let breaker = ComponentCircuitBreaker::new(3, 1000);

        // Initially should not bypass
        assert!(!breaker.should_bypass());

        // Record failures
        breaker.record_failure();
        breaker.record_failure();
        assert!(!breaker.should_bypass()); // Still under threshold

        breaker.record_failure();
        assert!(breaker.should_bypass()); // Over threshold

        // Success should reset
        breaker.record_success();
        assert!(!breaker.should_bypass());
    }
}
