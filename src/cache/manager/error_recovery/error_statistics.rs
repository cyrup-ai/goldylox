//! Error statistics tracking and analysis
//!
//! Provides comprehensive error tracking, burst detection, and
//! statistical analysis for the error recovery system.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crossbeam_utils::CachePadded;

use crate::telemetry::cache::manager::error_recovery::types::*;

/// Error statistics tracking
#[derive(Debug)]
pub struct ErrorStatistics {
    /// Total errors by type
    error_counts: CachePadded<[AtomicU64; 16]>, // Per error type
    /// Error rates (errors per second)
    error_rates: CachePadded<[AtomicU32; 16]>,
    /// Recovery attempt counts
    recovery_attempts: CachePadded<[AtomicU64; 8]>,
    /// Recovery success counts
    recovery_successes: CachePadded<[AtomicU64; 8]>,
    /// Mean time to recovery
    mttr_ns: CachePadded<AtomicU64>,
    /// Error burst detection
    burst_detector: ErrorBurstDetector,
}

impl ErrorStatistics {
    /// Create new error statistics
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
    #[inline(always)]
    pub fn record_error(&self, error_type: ErrorType) {
        let error_idx = error_type as usize;
        if error_idx < 16 {
            self.error_counts[error_idx].fetch_add(1, Ordering::Relaxed);
            self.burst_detector.record_error();
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

    /// Get total error count across all types
    #[inline(always)]
    pub fn get_total_error_count(&self) -> u64 {
        let mut total = 0;
        for i in 0..16 {
            total += self.error_counts[i].load(Ordering::Relaxed);
        }
        total
    }

    /// Record recovery attempt
    #[inline(always)]
    pub fn record_recovery_attempt(&self, strategy: RecoveryStrategy) {
        let strategy_idx = strategy as usize;
        if strategy_idx < 8 {
            self.recovery_attempts[strategy_idx].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record recovery success
    #[inline(always)]
    pub fn record_recovery_success(&self, strategy: RecoveryStrategy) {
        let strategy_idx = strategy as usize;
        if strategy_idx < 8 {
            self.recovery_successes[strategy_idx].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get recovery success rate for strategy
    #[inline(always)]
    pub fn get_recovery_success_rate(&self, strategy: RecoveryStrategy) -> f64 {
        let strategy_idx = strategy as usize;
        if strategy_idx < 8 {
            let attempts = self.recovery_attempts[strategy_idx].load(Ordering::Relaxed);
            let successes = self.recovery_successes[strategy_idx].load(Ordering::Relaxed);

            if attempts > 0 {
                successes as f64 / attempts as f64
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Check if system is in error burst
    #[inline(always)]
    pub fn is_in_burst(&self) -> bool {
        self.burst_detector.in_burst.load(Ordering::Relaxed)
    }

    /// Get mean time to recovery in nanoseconds
    #[inline(always)]
    pub fn get_mttr_ns(&self) -> u64 {
        self.mttr_ns.load(Ordering::Relaxed)
    }

    /// Update mean time to recovery
    #[inline]
    pub fn update_mttr(&self, recovery_time_ns: u64) {
        // Use exponential moving average for MTTR calculation
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

    /// Reset all statistics
    #[inline]
    pub fn reset_statistics(&self) {
        for i in 0..16 {
            self.error_counts[i].store(0, Ordering::Relaxed);
            self.error_rates[i].store(0, Ordering::Relaxed);
        }
        for i in 0..8 {
            self.recovery_attempts[i].store(0, Ordering::Relaxed);
            self.recovery_successes[i].store(0, Ordering::Relaxed);
        }
        self.mttr_ns.store(0, Ordering::Relaxed);
        self.burst_detector.in_burst.store(false, Ordering::Relaxed);
    }
}
