//! Error statistics tracking with atomic operations for thread safety
//!
//! Provides comprehensive error tracking with high-performance atomic counters,
//! error rate calculations, and integration with the existing error recovery systems.

use crate::cache::types::statistics::multi_tier::ErrorType;
use crossbeam_utils::CachePadded;
use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe error statistics tracker with atomic precision
#[derive(Debug)]
pub struct ErrorStatistics {
    /// Error counts per type (atomic for lock-free updates)
    /// Index maps to ErrorType discriminant values
    error_counts: [CachePadded<AtomicU64>; 12], // Updated for new ErrorType variants
    /// Total error count across all types
    total_errors: CachePadded<AtomicU64>,
    /// Total operations count for error rate calculation
    total_operations: CachePadded<AtomicU64>,
    /// Last error timestamp (nanoseconds since epoch)
    last_error_time: CachePadded<AtomicU64>,
    /// Error burst detection counter
    recent_error_count: CachePadded<AtomicU64>,
    /// Sliding window start time for burst detection
    window_start_time: CachePadded<AtomicU64>,
}

impl ErrorStatistics {
    /// Create new error statistics tracker
    pub fn new() -> Self {
        Self {
            error_counts: [
                CachePadded::new(AtomicU64::new(0)), // Timeout
                CachePadded::new(AtomicU64::new(0)), // OutOfMemory
                CachePadded::new(AtomicU64::new(0)), // MemoryAllocationFailure
                CachePadded::new(AtomicU64::new(0)), // Serialization
                CachePadded::new(AtomicU64::new(0)), // CoherenceViolation
                CachePadded::new(AtomicU64::new(0)), // SimdFailure
                CachePadded::new(AtomicU64::new(0)), // TierTransition
                CachePadded::new(AtomicU64::new(0)), // WorkerFailure
                CachePadded::new(AtomicU64::new(0)), // CorruptedData
                CachePadded::new(AtomicU64::new(0)), // DiskIOError
                CachePadded::new(AtomicU64::new(0)), // GenericError
                CachePadded::new(AtomicU64::new(0)), // Reserved for future expansion
            ],
            total_errors: CachePadded::new(AtomicU64::new(0)),
            total_operations: CachePadded::new(AtomicU64::new(0)),
            last_error_time: CachePadded::new(AtomicU64::new(0)),
            recent_error_count: CachePadded::new(AtomicU64::new(0)),
            window_start_time: CachePadded::new(AtomicU64::new(0)),
        }
    }

    /// Record an error of specific type
    pub fn record_error(&self, error_type: ErrorType) {
        let error_idx = self.error_type_to_index(error_type);

        // Update error counts atomically
        self.error_counts[error_idx].fetch_add(1, Ordering::Relaxed);
        self.total_errors.fetch_add(1, Ordering::Relaxed);

        // Update timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.last_error_time.store(now, Ordering::Relaxed);

        // Update burst detection
        self.update_burst_detection(now);

        log::warn!(
            "Error recorded: {:?} (total: {})",
            error_type,
            self.total_errors.load(Ordering::Relaxed)
        );
    }

    /// Record a successful operation
    pub fn record_operation(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Get error count for specific type
    pub fn get_error_count(&self, error_type: ErrorType) -> u64 {
        let error_idx = self.error_type_to_index(error_type);
        self.error_counts[error_idx].load(Ordering::Relaxed)
    }

    /// Get total error count
    pub fn get_total_errors(&self) -> u64 {
        self.total_errors.load(Ordering::Relaxed)
    }

    /// Get total operations count
    pub fn get_total_operations(&self) -> u64 {
        self.total_operations.load(Ordering::Relaxed)
    }

    /// Calculate current error rate (0.0 to 1.0)
    pub fn get_error_rate(&self) -> f64 {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        if total_ops == 0 {
            return 0.0;
        }
        let total_errors = self.total_errors.load(Ordering::Relaxed);
        total_errors as f64 / total_ops as f64
    }

    /// Get error distribution as normalized percentages
    pub fn get_error_distribution(&self) -> [f64; 12] {
        let total = self.total_errors.load(Ordering::Relaxed);
        if total == 0 {
            return [0.0; 12];
        }

        let mut distribution = [0.0; 12];
        for (i, dist_item) in distribution.iter_mut().enumerate() {
            let count = self.error_counts[i].load(Ordering::Relaxed);
            *dist_item = count as f64 / total as f64;
        }
        distribution
    }

    /// Check if system is experiencing error burst
    pub fn is_error_burst(&self) -> bool {
        self.recent_error_count.load(Ordering::Relaxed) > 10 // More than 10 errors in window
    }

    /// Get time since last error (nanoseconds)
    pub fn time_since_last_error(&self) -> Option<u64> {
        let last_error = self.last_error_time.load(Ordering::Relaxed);
        if last_error == 0 {
            return None;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Some(now.saturating_sub(last_error))
    }

    /// Reset all error statistics
    pub fn reset(&self) {
        for counter in &self.error_counts {
            counter.store(0, Ordering::Relaxed);
        }
        self.total_errors.store(0, Ordering::Relaxed);
        self.total_operations.store(0, Ordering::Relaxed);
        self.last_error_time.store(0, Ordering::Relaxed);
        self.recent_error_count.store(0, Ordering::Relaxed);
        self.window_start_time.store(0, Ordering::Relaxed);

        log::info!("Error statistics reset");
    }

    /// Convert ErrorType to array index
    fn error_type_to_index(&self, error_type: ErrorType) -> usize {
        match error_type {
            ErrorType::Timeout => 0,
            ErrorType::OutOfMemory => 1,
            ErrorType::MemoryAllocationFailure => 2,
            ErrorType::Serialization => 3,
            ErrorType::CoherenceViolation => 4,
            ErrorType::SimdFailure => 5,
            ErrorType::TierTransition => 6,
            ErrorType::WorkerFailure => 7,
            ErrorType::CorruptedData => 8,
            ErrorType::DiskIOError => 9,
            ErrorType::GenericError => 10,
        }
    }

    /// Update burst detection sliding window
    fn update_burst_detection(&self, now: u64) {
        const WINDOW_SIZE_NS: u64 = 60_000_000_000; // 60 seconds

        let window_start = self.window_start_time.load(Ordering::Relaxed);

        // Check if we need to reset the window
        if now.saturating_sub(window_start) > WINDOW_SIZE_NS {
            self.window_start_time.store(now, Ordering::Relaxed);
            self.recent_error_count.store(1, Ordering::Relaxed);
        } else {
            self.recent_error_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Default for ErrorStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ErrorStatistics {
    fn clone(&self) -> Self {
        let new_stats = Self::new();

        // Copy current values atomically
        for (i, counter) in self.error_counts.iter().enumerate() {
            new_stats.error_counts[i].store(counter.load(Ordering::Relaxed), Ordering::Relaxed);
        }

        new_stats
            .total_errors
            .store(self.total_errors.load(Ordering::Relaxed), Ordering::Relaxed);
        new_stats.total_operations.store(
            self.total_operations.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        new_stats.last_error_time.store(
            self.last_error_time.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        new_stats.recent_error_count.store(
            self.recent_error_count.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        new_stats.window_start_time.store(
            self.window_start_time.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );

        new_stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recording() {
        let stats = ErrorStatistics::new();

        // Record some errors
        stats.record_error(ErrorType::Timeout);
        stats.record_error(ErrorType::OutOfMemory);
        stats.record_error(ErrorType::Timeout);

        assert_eq!(stats.get_error_count(ErrorType::Timeout), 2);
        assert_eq!(stats.get_error_count(ErrorType::OutOfMemory), 1);
        assert_eq!(stats.get_total_errors(), 3);
    }

    #[test]
    fn test_error_rate_calculation() {
        let stats = ErrorStatistics::new();

        // Record operations and errors
        for _ in 0..100 {
            stats.record_operation();
        }
        for _ in 0..5 {
            stats.record_error(ErrorType::GenericError);
        }

        let error_rate = stats.get_error_rate();
        assert!((error_rate - 0.05).abs() < 0.001); // ~5% error rate
    }

    #[test]
    fn test_reset() {
        let stats = ErrorStatistics::new();

        stats.record_error(ErrorType::DiskIOError);
        stats.record_operation();

        assert_eq!(stats.get_total_errors(), 1);
        assert_eq!(stats.get_total_operations(), 1);

        stats.reset();

        assert_eq!(stats.get_total_errors(), 0);
        assert_eq!(stats.get_total_operations(), 0);
    }
}
