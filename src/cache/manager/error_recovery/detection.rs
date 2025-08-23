//! Error detection mechanisms and pattern recognition
//!
//! This module handles error pattern detection, health monitoring, and threshold management.

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::types::{
    DetectionState, ErrorPattern, ErrorThresholds, ErrorType, HealthCheckConfig, HealthStatus,
    PatternMatchingStats,
};

/// Error detection mechanisms
#[derive(Debug)]
pub struct ErrorDetector {
    /// Error rate thresholds
    pub error_thresholds: ErrorThresholds,
    /// Error pattern recognition
    pub pattern_detector: super::types::ErrorPatternDetector,
    /// Health check configuration
    pub health_check_config: HealthCheckConfig,
    /// Detection state
    pub detection_state: DetectionState,
}

impl ErrorDetector {
    /// Create new error detector
    pub fn new() -> Self {
        Self {
            error_thresholds: ErrorThresholds::default(),
            pattern_detector: super::types::ErrorPatternDetector::new(),
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
    pub fn perform_health_check(&self) -> HealthStatus {
        let now = Instant::now();
        let last_check = self.detection_state.last_health_check.load();

        if now.duration_since(last_check) >= self.health_check_config.check_interval {
            let status = self.evaluate_system_health();
            self.detection_state.health_status.store(status);
            self.detection_state.last_health_check.store(now);
            status
        } else {
            self.detection_state.health_status.load()
        }
    }

    /// Update error thresholds
    pub fn update_thresholds(
        &self,
        max_error_rate: u32,
        burst_threshold: u32,
        critical_threshold: u32,
    ) {
        self.error_thresholds
            .max_error_rate
            .store(max_error_rate, Ordering::Relaxed);
        self.error_thresholds
            .burst_threshold
            .store(burst_threshold, Ordering::Relaxed);
        self.error_thresholds
            .critical_threshold
            .store(critical_threshold, Ordering::Relaxed);
    }

    /// Check if error rate exceeds threshold
    pub fn exceeds_error_rate_threshold(&self, current_rate: u32) -> bool {
        current_rate > self.error_thresholds.max_error_rate.load(Ordering::Relaxed)
    }

    /// Check if in critical error state
    pub fn is_critical_error_state(&self, error_count: u32) -> bool {
        error_count
            > self
                .error_thresholds
                .critical_threshold
                .load(Ordering::Relaxed)
    }

    /// Set detection sensitivity
    pub fn set_sensitivity(&self, sensitivity: f32) {
        let clamped = sensitivity.clamp(0.0, 1.0);
        self.detection_state.sensitivity.store(clamped);
    }

    /// Get detection sensitivity
    pub fn get_sensitivity(&self) -> f32 {
        self.detection_state.sensitivity.load()
    }

    /// Enable/disable health checks
    pub fn set_health_check_enabled(&self, enabled: bool) {
        self.health_check_config
            .enabled
            .store(enabled, Ordering::Relaxed);
    }

    /// Check if health checks are enabled
    pub fn is_health_check_enabled(&self) -> bool {
        self.health_check_config.enabled.load(Ordering::Relaxed)
    }

    fn evaluate_system_health(&self) -> HealthStatus {
        // Simplified health evaluation - can be expanded with actual metrics
        let sensitivity = self.detection_state.sensitivity.load();

        // Basic health assessment based on detection sensitivity
        if sensitivity > 0.8 {
            HealthStatus::Healthy
        } else if sensitivity > 0.5 {
            HealthStatus::Degraded
        } else if sensitivity > 0.2 {
            HealthStatus::Critical
        } else {
            HealthStatus::Failed
        }
    }
}

impl super::types::ErrorPatternDetector {
    pub fn new() -> Self {
        Self {
            pattern_window: std::time::Duration::from_secs(60),
            confidence_threshold: crossbeam_utils::atomic::AtomicCell::new(0.8),
            known_patterns: Vec::new(),
            matching_state: super::types::PatternMatchingState::new(),
        }
    }

    pub fn match_pattern(&self, _error_type: ErrorType) -> Option<&ErrorPattern> {
        // Simplified pattern matching - can be expanded with actual pattern recognition
        None
    }

    /// Add a new error pattern to the detector
    pub fn add_pattern(&mut self, pattern: ErrorPattern) {
        self.known_patterns.push(pattern);
    }

    /// Remove a pattern by ID
    pub fn remove_pattern(&mut self, pattern_id: u32) {
        self.known_patterns.retain(|p| p.pattern_id != pattern_id);
    }

    /// Get pattern confidence threshold
    pub fn get_confidence_threshold(&self) -> f32 {
        self.confidence_threshold.load()
    }

    /// Set pattern confidence threshold
    pub fn set_confidence_threshold(&self, threshold: f32) {
        let clamped = threshold.clamp(0.0, 1.0);
        self.confidence_threshold.store(clamped);
    }

    /// Get number of known patterns
    pub fn pattern_count(&self) -> usize {
        self.known_patterns.len()
    }

    /// Clear all patterns
    pub fn clear_patterns(&mut self) {
        self.known_patterns.clear();
        self.matching_state.active_matches.clear();
        self.matching_state.pattern_history.clear();
    }

    /// Get pattern matching statistics
    pub fn get_matching_stats(&self) -> &PatternMatchingStats {
        &self.matching_state.matching_stats
    }
}
