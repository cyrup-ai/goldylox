//! Error statistics tracking and analysis
//!
//! This module handles comprehensive error statistics collection, analysis, and reporting.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crossbeam_utils::CachePadded;

use super::types::{ErrorBurstDetector, ErrorType, RecoveryStrategy};

/// Error statistics tracking with comprehensive analytics
#[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
#[derive(Debug)]
pub struct ErrorStatistics {
    /// Total errors by type (private for better encapsulation)
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    error_counts: CachePadded<[AtomicU64; 16]>, // Per error type
    /// Error rates (errors per second)
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    error_rates: CachePadded<[AtomicU32; 16]>,
    /// Recovery attempt counts
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    recovery_attempts: CachePadded<[AtomicU64; 8]>,
    /// Recovery success counts
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    recovery_successes: CachePadded<[AtomicU64; 8]>,
    /// Mean time to recovery
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    mttr_ns: CachePadded<AtomicU64>,
    /// Error burst detection
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    burst_detector: ErrorBurstDetector,
}

#[allow(dead_code)] // Error recovery - comprehensive error statistics library for monitoring and analytics
impl ErrorStatistics {
    /// Create new error statistics tracker with optimized const initialization
    pub fn new() -> Self {
        Self {
            error_counts: CachePadded::new([const { AtomicU64::new(0) }; 16]),
            error_rates: CachePadded::new([const { AtomicU32::new(0) }; 16]),
            recovery_attempts: CachePadded::new([const { AtomicU64::new(0) }; 8]),
            recovery_successes: CachePadded::new([const { AtomicU64::new(0) }; 8]),
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

    /// Record recovery attempt (type-safe with RecoveryStrategy enum)
    #[allow(dead_code)] // Error recovery - record_recovery_attempt used in recovery attempt tracking
    #[inline]
    pub fn record_recovery_attempt(&self, strategy: RecoveryStrategy) {
        let strategy_idx = strategy as usize;
        if strategy_idx < 8 {
            self.recovery_attempts[strategy_idx].fetch_add(1, Ordering::Relaxed);
        }
    }



    /// Record recovery success (type-safe with RecoveryStrategy enum)
    #[inline]
    pub fn record_recovery_success(&self, strategy: RecoveryStrategy, recovery_time_ns: u64) {
        let strategy_idx = strategy as usize;
        if strategy_idx < 8 {
            self.recovery_successes[strategy_idx].fetch_add(1, Ordering::Relaxed);
            self.update_mttr_internal(recovery_time_ns);
        }
    }



    /// Internal MTTR update logic (shared between methods)
    #[inline]
    fn update_mttr_internal(&self, recovery_time_ns: u64) {
        const ALPHA: f64 = 0.1; // EMA smoothing factor
        
        let current_mttr = self.mttr_ns.load(Ordering::Relaxed);
        let new_mttr = if current_mttr == 0 {
            recovery_time_ns
        } else {
            // Exponential moving average: new = α * current + (1-α) * old
            ((ALPHA * recovery_time_ns as f64) + ((1.0 - ALPHA) * current_mttr as f64)) as u64
        };
        self.mttr_ns.store(new_mttr, Ordering::Relaxed);
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

    /// Get recovery attempt count for strategy (type-safe)
    #[inline(always)]
    pub fn get_recovery_attempts(&self, strategy: RecoveryStrategy) -> u64 {
        let strategy_idx = strategy as usize;
        if strategy_idx < 8 {
            self.recovery_attempts[strategy_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get recovery attempt count for strategy (by index)
    #[inline(always)]
    pub fn get_recovery_attempts_by_index(&self, strategy_idx: usize) -> u64 {
        if strategy_idx < 8 {
            self.recovery_attempts[strategy_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get recovery success count for strategy (type-safe)
    #[inline(always)]
    pub fn get_recovery_successes(&self, strategy: RecoveryStrategy) -> u64 {
        let strategy_idx = strategy as usize;
        if strategy_idx < 8 {
            self.recovery_successes[strategy_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get recovery success count for strategy (by index)
    #[inline(always)]
    pub fn get_recovery_successes_by_index(&self, strategy_idx: usize) -> u64 {
        if strategy_idx < 8 {
            self.recovery_successes[strategy_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get recovery success rate for strategy (type-safe)
    #[inline(always)]
    pub fn get_recovery_success_rate(&self, strategy: RecoveryStrategy) -> f64 {
        let strategy_idx = strategy as usize;
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

    /// Get recovery success rate for strategy (legacy usize interface)
    #[inline(always)]
    pub fn get_recovery_success_rate_by_index(&self, strategy_idx: usize) -> f64 {
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

    /// Get error rate for specific error type
    #[inline(always)]
    pub fn get_error_rate(&self, error_type: ErrorType) -> u32 {
        let error_idx = error_type as usize;
        if error_idx < 16 {
            self.error_rates[error_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Update error rate for specific error type
    #[inline]
    pub fn update_error_rate(&self, error_type: ErrorType, rate: u32) {
        let error_idx = error_type as usize;
        if error_idx < 16 {
            self.error_rates[error_idx].store(rate, Ordering::Relaxed);
        }
    }

    /// Check if system is in error burst
    #[inline(always)]
    pub fn is_in_burst(&self) -> bool {
        self.burst_detector.in_burst.load(Ordering::Relaxed)
    }

    /// Reset all statistics
    pub fn reset_statistics(&self) {
        // Reset error counts and rates
        for i in 0..16 {
            self.error_counts[i].store(0, Ordering::Relaxed);
            self.error_rates[i].store(0, Ordering::Relaxed);
        }

        // Reset recovery statistics
        for i in 0..8 {
            self.recovery_attempts[i].store(0, Ordering::Relaxed);
            self.recovery_successes[i].store(0, Ordering::Relaxed);
        }

        // Reset MTTR and burst detector
        self.mttr_ns.store(0, Ordering::Relaxed);
        self.burst_detector.in_burst.store(false, Ordering::Relaxed);
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
            if error_count > 0
                && let Some(error_type) = index_to_error_type(i)
            {
                errors.push((error_type, error_count));
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

        (recovery_rate - burst_penalty - error_penalty).clamp(0.0, 1.0)
    }
}

// Helper function to convert index to ErrorType
#[allow(dead_code)] // Error recovery - index_to_error_type used in error type conversion utilities
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

impl Clone for ErrorStatistics {
    fn clone(&self) -> Self {
        // Create a new instance and copy atomic values
        let new_stats = Self::new();
        
        // Copy error counts
        for i in 0..16 {
            let value = self.error_counts[i].load(Ordering::Relaxed);
            new_stats.error_counts[i].store(value, Ordering::Relaxed);
        }
        
        // Copy error rates  
        for i in 0..16 {
            let value = self.error_rates[i].load(Ordering::Relaxed);
            new_stats.error_rates[i].store(value, Ordering::Relaxed);
        }
        
        // Copy recovery attempts
        for i in 0..8 {
            let value = self.recovery_attempts[i].load(Ordering::Relaxed);
            new_stats.recovery_attempts[i].store(value, Ordering::Relaxed);
        }
        
        // Copy recovery successes
        for i in 0..8 {
            let value = self.recovery_successes[i].load(Ordering::Relaxed);
            new_stats.recovery_successes[i].store(value, Ordering::Relaxed);
        }
        
        // Copy mean time to recovery
        let mttr_value = self.mttr_ns.load(Ordering::Relaxed);
        new_stats.mttr_ns.store(mttr_value, Ordering::Relaxed);
        
        // burst_detector is already cloned via the new() constructor
        new_stats
    }
}
