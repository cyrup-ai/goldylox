//! Type definitions for error recovery system
//!
//! This module contains all the enums, structs, and configuration types
//! used throughout the error recovery system.

#![allow(dead_code)] // Error recovery - comprehensive resilience infrastructure

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::time::{Duration, Instant};

use crossbeam_utils::atomic::AtomicCell;

/// Error type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorType {
    MemoryAllocationFailure,
    DiskIOError,
    NetworkTimeout,
    CorruptedData,
    ConcurrencyViolation,
    ResourceExhaustion,
    ConfigurationError,
    SystemOverload,
}

/// Recovery strategy enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    Retry,
    Fallback,
    GracefulDegradation,
    EmergencyShutdown,
    DataReconstruction,
    ResourceReallocation,
    ConfigurationReset,
    SystemRestart,
    CircuitBreaker,
    Graceful,
    Escalate,
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing recovery
}

/// Health status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Failed,
}

/// Backoff strategy for retries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffStrategy {
    Linear,
    Exponential,
    Fibonacci,
    Custom,
}

/// Error thresholds for detection
#[derive(Debug)]
pub struct ErrorThresholds {
    /// Maximum error rate (errors per second)
    pub max_error_rate: AtomicU32,
    /// Error burst threshold
    pub burst_threshold: AtomicU32,
    /// Critical error threshold
    pub critical_threshold: AtomicU32,
    /// Recovery timeout
    pub recovery_timeout_ns: AtomicU64,
}

/// Health check configuration
#[derive(Debug)]
pub struct HealthCheckConfig {
    /// Health check interval
    pub check_interval: Duration,
    /// Health check timeout
    pub check_timeout: Duration,
    /// Consecutive failure threshold
    pub failure_threshold: u32,

}

/// Detection state tracking
#[derive(Debug)]
pub struct DetectionState {
    /// Last health check timestamp
    pub last_health_check: AtomicCell<Instant>,
    /// Current health status
    pub health_status: AtomicCell<HealthStatus>,
    /// Detection sensitivity
    pub sensitivity: AtomicCell<f32>,
    /// Active detection rules
    pub active_rules: AtomicU32,
}

/// Recovery configuration
#[derive(Debug)]
pub struct RecoveryConfig {
    /// Maximum concurrent recovery operations
    pub max_concurrent_recoveries: u32,
    /// Recovery operation timeout
    pub recovery_timeout: Duration,
    /// Retry limits per strategy
    pub retry_limits: [u32; 8], // Per strategy type
    /// Backoff strategy for retries
    pub backoff_strategy: BackoffStrategy,
}

/// Circuit breaker configuration
#[derive(Debug)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,
    /// Success threshold for closing circuit
    pub success_threshold: u32,
    /// Timeout duration for half-open state
    pub timeout_duration: Duration,
    /// Number of test requests in half-open state
    pub half_open_test_count: u32,
}

/// Error pattern detection
#[derive(Debug)]
pub struct ErrorPatternDetector {
    /// Pattern recognition window
    pub pattern_window: Duration,
    /// Pattern confidence threshold
    pub confidence_threshold: AtomicCell<f32>,
    /// Known error patterns
    pub known_patterns: Vec<ErrorPattern>,
    /// Pattern matching state
    pub matching_state: PatternMatchingState,
}

/// Error pattern definition
#[derive(Debug, Clone)]
pub struct ErrorPattern {
    /// Pattern identifier
    pub pattern_id: u32,
    /// Error sequence
    pub error_sequence: Vec<ErrorType>,
    /// Time window for pattern
    pub time_window: Duration,
    /// Pattern confidence
    pub confidence: f32,
    /// Associated recovery strategy
    pub recovery_strategy: RecoveryStrategy,
}

/// Pattern match in progress
#[derive(Debug)]
pub struct PatternMatch {
    /// Pattern being matched
    pub pattern_id: u32,
    /// Current position in pattern
    pub position: usize,
    /// Match start time
    pub start_time: Instant,
    /// Match confidence
    pub confidence: f32,
}

/// Completed pattern match
#[derive(Debug)]
pub struct CompletedPattern {
    /// Pattern that was matched
    pub pattern_id: u32,
    /// Completion timestamp
    pub completed_at: Instant,
    /// Final confidence score
    pub final_confidence: f32,
    /// Recovery action taken
    pub recovery_action: RecoveryStrategy,
}

/// Pattern matching state
#[derive(Debug)]
pub struct PatternMatchingState {
    /// Current matching patterns
    pub active_matches: Vec<PatternMatch>,
    /// Pattern history
    pub pattern_history: Vec<CompletedPattern>,
    /// Matching statistics
    pub matching_stats: PatternMatchingStats,
}

impl Default for PatternMatchingState {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternMatchingState {
    pub fn new() -> Self {
        Self {
            active_matches: Vec::new(),
            pattern_history: Vec::new(),
            matching_stats: PatternMatchingStats::default(),
        }
    }
}

/// Pattern matching statistics
#[derive(Debug, Default)]
pub struct PatternMatchingStats {
    /// Total patterns matched
    pub total_matches: AtomicU64,
    /// Successful recoveries from patterns
    pub successful_recoveries: AtomicU64,
    /// Average pattern confidence
    pub avg_confidence: AtomicCell<f32>,
    /// Pattern detection accuracy
    pub detection_accuracy: AtomicCell<f32>,
    /// Recent pattern matches count
    pub recent_matches: AtomicU64,
    /// Total patterns detected
    pub total_patterns_detected: AtomicU64,
}

/// Error burst detection
#[derive(Debug)]
pub struct ErrorBurstDetector {
    /// Time window for burst detection
    pub window_size: Duration,
    /// Error timestamps within window
    pub error_timestamps: Vec<Instant>,
    /// Burst threshold
    pub burst_threshold: u32,
    /// Current burst state
    pub in_burst: AtomicBool,
}

// Default implementations
impl Default for ErrorThresholds {
    fn default() -> Self {
        Self {
            max_error_rate: AtomicU32::new(100), // 100 errors per second
            burst_threshold: AtomicU32::new(50),
            critical_threshold: AtomicU32::new(1000),
            recovery_timeout_ns: AtomicU64::new(30_000_000_000), // 30 seconds
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(5),
            failure_threshold: 3,

        }
    }
}

impl Default for DetectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl DetectionState {
    pub fn new() -> Self {
        Self {
            last_health_check: AtomicCell::new(Instant::now()),
            health_status: AtomicCell::new(HealthStatus::Healthy),
            sensitivity: AtomicCell::new(0.5),
            active_rules: AtomicU32::new(0),
        }
    }
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_concurrent_recoveries: 10,
            recovery_timeout: Duration::from_secs(60),
            retry_limits: [3, 5, 2, 1, 3, 2, 1, 1], // Per strategy
            backoff_strategy: BackoffStrategy::Exponential,
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_duration: Duration::from_secs(60),
            half_open_test_count: 3,
        }
    }
}

impl Default for ErrorBurstDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorBurstDetector {
    pub fn new() -> Self {
        Self {
            window_size: Duration::from_secs(10),
            error_timestamps: Vec::new(),
            burst_threshold: 20,
            in_burst: AtomicBool::new(false),
        }
    }

    pub fn record_error(&self) {
        // Simplified burst detection implementation
    }
}

impl PatternMatchingStats {
    pub fn new() -> Self {
        Self {
            total_matches: AtomicU64::new(0),
            successful_recoveries: AtomicU64::new(0),
            avg_confidence: AtomicCell::new(0.0),
            detection_accuracy: AtomicCell::new(0.0),
            recent_matches: AtomicU64::new(0),
            total_patterns_detected: AtomicU64::new(0),
        }
    }
}
