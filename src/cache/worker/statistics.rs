//! Worker statistics implementation and utilities
//!
//! This module implements the WorkerStats struct with methods for
//! calculating derived metrics and health checks.

use std::time::Duration;

use super::types::WorkerStats;

impl WorkerStats {
    /// Create new worker statistics
    pub fn new() -> Self {
        Self {
            tasks_processed: 0,
            tasks_queued: 0,
            avg_task_time_ns: 0,
            uptime_seconds: 0,
            last_maintenance: None,
            promotions: 0,
            demotions: 0,
            cleanups: 0,
            tier_transitions: 0,
            last_tier_check: None,
            entries_cleaned: 0,
            last_cleanup: None,
        }
    }

    /// Calculate tasks per second
    pub fn tasks_per_second(&self) -> f64 {
        if self.uptime_seconds > 0 {
            self.tasks_processed as f64 / self.uptime_seconds as f64
        } else {
            0.0
        }
    }

    /// Calculate average task time in milliseconds
    pub fn avg_task_time_ms(&self) -> f64 {
        self.avg_task_time_ns as f64 / 1_000_000.0
    }

    /// Check if worker is healthy
    pub fn is_healthy(&self) -> bool {
        // Worker is healthy if:
        // - Task queue isn't too large
        // - Average task time is reasonable
        // - Recent maintenance has occurred

        let queue_ok = self.tasks_queued < 1000;
        let task_time_ok = self.avg_task_time_ns < 10_000_000; // 10ms
        let maintenance_ok = self
            .last_maintenance
            .map_or(true, |t| t.elapsed() < Duration::from_secs(300)); // 5 minutes

        queue_ok && task_time_ok && maintenance_ok
    }
}
