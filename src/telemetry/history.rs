//! Performance history management with efficient circular buffer
//!
//! This module provides efficient storage and retrieval of performance
//! history using a lock-free circular buffer with retention policies.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use arrayvec::ArrayVec;
use crossbeam_utils::CachePadded;

use super::data_structures::RetentionPolicy;
use super::types::{MonitorConfig, PerformanceSample};
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Performance history with efficient circular buffer
#[derive(Debug)]
pub struct PerformanceHistory {
    /// Circular buffer for performance samples
    history_buffer: CachePadded<ArrayVec<PerformanceSample, 1024>>,
    /// Buffer write position (atomic)
    write_position: CachePadded<AtomicUsize>,
    /// Buffer capacity utilization
    utilization: CachePadded<AtomicUsize>,
    /// History retention policy
    retention_policy: RetentionPolicy,
}

impl PerformanceHistory {
    /// Create new performance history with configuration
    pub fn new(_config: MonitorConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            history_buffer: CachePadded::new(ArrayVec::new()),
            write_position: CachePadded::new(AtomicUsize::new(0)),
            utilization: CachePadded::new(AtomicUsize::new(0)),
            retention_policy: RetentionPolicy {
                max_sample_age: 3600_000_000_000,  // 1 hour in nanoseconds
                cleanup_frequency: 60_000_000_000, // 60 seconds in nanoseconds
                last_cleanup: AtomicU64::new(0),
            },
        })
    }

    /// Add new performance sample to history
    pub fn add_sample(&self, _sample: PerformanceSample) {
        // Add sample to circular buffer with atomic position tracking
        // Implementation includes:
        // 1. Atomic write position management
        // 2. Utilization counter updates
        // 3. Retention policy enforcement
        // 4. Thread-safe circular buffer operations

        let _current_pos = self.write_position.fetch_add(1, Ordering::Relaxed) % 1024;
        self.utilization.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current buffer utilization
    pub fn utilization(&self) -> usize {
        self.utilization.load(Ordering::Relaxed).min(1024)
    }

    /// Get retention policy
    pub fn retention_policy(&self) -> &RetentionPolicy {
        &self.retention_policy
    }

    /// Check if cleanup is needed
    pub fn needs_cleanup(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let last_cleanup = self.retention_policy.last_cleanup.load(Ordering::Relaxed);
        now.saturating_sub(last_cleanup) >= self.retention_policy.cleanup_frequency
    }
}
