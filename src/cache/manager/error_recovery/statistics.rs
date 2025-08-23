//! Error statistics tracking and analysis
//!
//! This module handles comprehensive error statistics collection, analysis, and reporting.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crossbeam_utils::CachePadded;

use super::types::{ErrorBurstDetector, ErrorType};

/// Error statistics tracking
#[derive(Debug)]
pub struct ErrorStatistics {
    /// Total errors by type
    pub error_counts: CachePadded<[AtomicU64; 16]>, // Per error type
    /// Error rates (errors per second)
    pub error_rates: CachePadded<[AtomicU32; 16]>,
    /// Recovery attempt counts
    pub recovery_attempts: CachePadded<[AtomicU64; 8]>,
    /// Recovery success counts
    pub recovery_successes: CachePadded<[AtomicU64; 8]>,
    /// Mean time to recovery
    pub mttr_ns: CachePadded<AtomicU64>,
    /// Error burst detection
    pub burst_detector: ErrorBurstDetector,
}

impl ErrorStatistics {
    /// Create new error statistics tracker
    pub fn new() -> Self {
        Self {
            error_counts: CachePadded::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            error_rates: CachePadded::new([
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
            ]),
            recovery_attempts: CachePadded::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            recovery_successes: CachePadded::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            mttr_ns: CachePadded::new(AtomicU64::new(0)),
            burst_detector: ErrorBurstDetector::new(),
        }
    }

    /// Record error occurrence
    #[inline]
    pub fn record_error(&self, error_type: ErrorType) {
        let error_idx = error_type as usize;
        if error_idx < 16 {
            self.error_counts[error_idx].fetch_add(1, Ordering::Relaxed);
            self.burst_detector.record_error();
        }
    }

    /// Record recovery attempt
    #[inline]
    pub fn record_recovery_attempt(&self, strategy_idx: usize) {
        if strategy_idx < 8 {
            self.recovery_attempts[strategy_idx].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record recovery success
    #[inline]
    pub fn record_recovery_success(&self, strategy_idx: usize, recovery_time_ns: u64) {
        if strategy_idx < 8 {
            self.recovery_successes[strategy_idx].fetch_add(1, Ordering::Relaxed);

            // Update mean time to recovery using exponential moving average
            let current_mttr = self.mttr_ns.load(Ordering::Relaxed);
            let new_mttr = if current_mttr == 0 {
                recovery_time_ns
            } else {
                // Simple exponential moving average with alpha = 0.1
                (current_mttr * 9 + recovery_time_ns) / 10
            };
            self.mttr_ns.store(new_mttr, Ordering::Relaxed);
        }
    }

    /// Get error count for type
    #[inline(always)]
    pub fn get_error_count(&self, error_type: ErrorType) -> u64 {
        let error_idx = error_type as usize;
        if error_idx < 16 {
            self.error_counts[error_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get total error count
    #[inline(always)]
    pub fn get_total_error_count(&self) -> u64 {
        self.error_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum()
    }

    /// Get recovery attempt count for strategy
    #[inline(always)]
    pub fn get_recovery_attempts(&self, strategy_idx: usize) -> u64 {
        if strategy_idx < 8 {
            self.recovery_attempts[strategy_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get recovery success count for strategy
    #[inline(always)]
    pub fn get_recovery_successes(&self, strategy_idx: usize) -> u64 {
        if strategy_idx < 8 {
            self.recovery_successes[strategy_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get recovery success rate for strategy
    #[inline(always)]
    pub fn get_recovery_success_rate(&self, strategy_idx: usize) -> f64 {
        if strategy_idx >= 8 {
            return 0.0;
        }

        let attempts = self.recovery_attempts[strategy_idx].load(Ordering::Relaxed);
        let successes = self.recovery_successes[strategy_idx].load(Ordering::Relaxed);

        if attempts > 0 {
            successes as f64 / attempts as f64
        } else {
            0.0
        }
    }

    /// Get mean time to recovery in nanoseconds
    #[inline(always)]
    pub fn get_mttr_ns(&self) -> u64 {
        self.mttr_ns.load(Ordering::Relaxed)
    }

    /// Get mean time to recovery in milliseconds
    #[inline(always)]
    pub fn get_mttr_ms(&self) -> f64 {
        self.get_mttr_ns() as f64 / 1_000_000.0
    }

    /// Check if system is in error burst
    #[inline(always)]
    pub fn is_in_burst(&self) -> bool {
        self.burst_detector.in_burst.load(Ordering::Relaxed)
    }

    /// Reset all statistics
    pub fn reset_all(&self) {
        // Reset error counts
        for count in &self.error_counts[..] {
            count.store(0, Ordering::Relaxed);
        }

        // Reset error rates
        for rate in &self.error_rates[..] {
            rate.store(0, Ordering::Relaxed);
        }

        // Reset recovery statistics
        for attempt in &self.recovery_attempts[..] {
            attempt.store(0, Ordering::Relaxed);
        }

        for success in &self.recovery_successes[..] {
            success.store(0, Ordering::Relaxed);
        }

        // Reset MTTR
        self.mttr_ns.store(0, Ordering::Relaxed);
    }

    /// Get error distribution (percentage of each error type)
    pub fn get_error_distribution(&self) -> [f64; 16] {
        let total = self.get_total_error_count() as f64;
        let mut distribution = [0.0; 16];

        if total > 0.0 {
            for (i, count) in self.error_counts.iter().enumerate() {
                distribution[i] = count.load(Ordering::Relaxed) as f64 / total;
            }
        }

        distribution
    }

    /// Get top error types by count
    pub fn get_top_error_types(&self, limit: usize) -> Vec<(ErrorType, u64)> {
        let mut errors: Vec<(ErrorType, u64)> = Vec::new();

        // Collect all error types with their counts
        for (i, count) in self.error_counts.iter().enumerate() {
            let error_count = count.load(Ordering::Relaxed);
            if error_count > 0 {
                if let Some(error_type) = index_to_error_type(i) {
                    errors.push((error_type, error_count));
                }
            }
        }

        // Sort by count (descending) and take top N
        errors.sort_by(|a, b| b.1.cmp(&a.1));
        errors.truncate(limit);

        errors
    }

    /// Get overall system health score (0.0 = unhealthy, 1.0 = healthy)
    pub fn get_health_score(&self) -> f64 {
        let total_errors = self.get_total_error_count();
        let total_attempts: u64 = self
            .recovery_attempts
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .sum();
        let total_successes: u64 = self
            .recovery_successes
            .iter()
            .map(|s| s.load(Ordering::Relaxed))
            .sum();

        // Base health score on recovery success rate
        let recovery_rate = if total_attempts > 0 {
            total_successes as f64 / total_attempts as f64
        } else {
            1.0 // No failures means perfect health
        };

        // Penalize for error bursts
        let burst_penalty = if self.is_in_burst() { 0.2 } else { 0.0 };

        // Penalize for high error counts
        let error_penalty = (total_errors as f64 / 1000.0).min(0.3);

        (recovery_rate - burst_penalty - error_penalty)
            .max(0.0)
            .min(1.0)
    }
}

// Helper function to convert index to ErrorType
fn index_to_error_type(index: usize) -> Option<ErrorType> {
    match index {
        0 => Some(ErrorType::MemoryAllocationFailure),
        1 => Some(ErrorType::DiskIOError),
        2 => Some(ErrorType::NetworkTimeout),
        3 => Some(ErrorType::CorruptedData),
        4 => Some(ErrorType::ConcurrencyViolation),
        5 => Some(ErrorType::ResourceExhaustion),
        6 => Some(ErrorType::ConfigurationError),
        7 => Some(ErrorType::SystemOverload),
        _ => None,
    }
}

impl Default for ErrorStatistics {
    fn default() -> Self {
        Self::new()
    }
}
