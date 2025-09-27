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

    /// Record a successful maintenance operation
    pub fn record_operation_success(
        &self,
        task: &super::types::CanonicalMaintenanceTask,
        execution_time_ns: u64,
    ) {
        use std::time::Instant;

        // Update counters atomically
        self.operations_executed.fetch_add(1, Ordering::Relaxed);
        self.total_maintenance_time_ns
            .fetch_add(execution_time_ns, Ordering::Relaxed);
        self.last_maintenance.store(Some(Instant::now()));

        // Update operation-specific counters based on task type
        match task {
            crate::cache::tier::warm::maintenance::MaintenanceTask::CleanupExpired { .. } => {
                self.cleanup_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::CompactStorage { .. } => {
                self.defrag_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::AnalyzePatterns { .. } => {
                self.rebuild_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::PerformEviction { .. } => {
                self.validation_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::OptimizeStructure {
                ..
            } => {
                self.optimization_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::UpdateStatistics { .. }
            | crate::cache::tier::warm::maintenance::MaintenanceTask::Evict { .. }
            | crate::cache::tier::warm::maintenance::MaintenanceTask::SyncTiers { .. } => {
                self.pattern_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::ValidateIntegrity {
                ..
            } => {
                self.validation_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::DefragmentMemory { .. } => {
                self.defrag_operations.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::tier::warm::maintenance::MaintenanceTask::UpdateMLModels { .. } => {
                self.optimization_operations.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record a failed maintenance operation
    pub fn record_operation_failure(&self, _task: &super::types::CanonicalMaintenanceTask) {
        self.failed_operations.fetch_add(1, Ordering::Relaxed);
        self.operations_executed.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total operations count
    #[allow(dead_code)] // Background statistics - total operation count API for monitoring
    pub fn get_total_operations(&self) -> u64 {
        self.operations_executed.load(Ordering::Relaxed)
    }

    /// Get average execution time in nanoseconds
    #[allow(dead_code)] // Background statistics - average execution time API for performance analysis
    pub fn get_average_execution_time_ns(&self) -> u64 {
        let total_ops = self.operations_executed.load(Ordering::Relaxed);
        let total_time = self.total_maintenance_time_ns.load(Ordering::Relaxed);
        if total_ops > 0 {
            total_time / total_ops
        } else {
            0
        }
    }

    /// Get operation breakdown statistics
    #[allow(dead_code)] // Background statistics - operation breakdown API for detailed maintenance analysis
    pub fn get_operation_breakdown(&self) -> OperationBreakdown {
        OperationBreakdown {
            cleanup_count: self.cleanup_operations.load(Ordering::Relaxed),
            defrag_count: self.defrag_operations.load(Ordering::Relaxed),
            rebuild_count: self.rebuild_operations.load(Ordering::Relaxed),
            validation_count: self.validation_operations.load(Ordering::Relaxed),
            optimization_count: self.optimization_operations.load(Ordering::Relaxed),
            pattern_count: self.pattern_operations.load(Ordering::Relaxed),
            failed_count: self.failed_operations.load(Ordering::Relaxed),
        }
    }
}

/// Operation breakdown statistics for detailed monitoring
#[derive(Debug, Clone)]
#[allow(dead_code)] // Background statistics - operation breakdown fields for comprehensive maintenance monitoring
pub struct OperationBreakdown {
    pub cleanup_count: u64,
    pub defrag_count: u64,
    pub rebuild_count: u64,
    pub validation_count: u64,
    pub optimization_count: u64,
    pub pattern_count: u64,
    pub failed_count: u64,
}

impl Default for OperationBreakdown {
    #[inline]
    fn default() -> Self {
        Self {
            cleanup_count: 0,
            defrag_count: 0,
            rebuild_count: 0,
            validation_count: 0,
            optimization_count: 0,
            pattern_count: 0,
            failed_count: 0,
        }
    }
}
