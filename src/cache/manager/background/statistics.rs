//! Statistics collection and reporting
//!
//! This module implements statistics tracking for maintenance operations
//! including completion rates, failure rates, and performance metrics.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam::atomic::AtomicCell;

use super::types::MaintenanceStats;

#[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
#[derive(Debug)]
pub struct BackgroundStatistics {
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    stats: MaintenanceStats,
}

impl MaintenanceStats {
    /// Create new maintenance statistics
    pub fn new() -> Self {
        Self {
            total_submitted: AtomicU64::new(0),
            operations_executed: AtomicU64::new(0),
            total_maintenance_time_ns: AtomicU64::new(0),
            cleanup_operations: AtomicU64::new(0),
            defrag_operations: AtomicU64::new(0),
            rebuild_operations: AtomicU64::new(0),
            validation_operations: AtomicU64::new(0),
            optimization_operations: AtomicU64::new(0),
            pattern_operations: AtomicU64::new(0),
            failed_operations: AtomicU64::new(0),
            last_maintenance: AtomicCell::new(None),
        }
    }

    /// Get failure rate (0.0 to 1.0)
    #[allow(dead_code)] // Background workers - failure_rate used in background operation failure analysis
    #[inline(always)]
    pub fn failure_rate(&self) -> f64 {
        let total = self.operations_executed.load(Ordering::Relaxed);
        let failed = self.failed_operations.load(Ordering::Relaxed);
        if total > 0 {
            failed as f64 / total as f64
        } else {
            0.0
        }
    }
}
