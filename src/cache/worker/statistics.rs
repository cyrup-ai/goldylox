//! Worker statistics implementation and utilities
//!
//! This module implements the WorkerStats struct with methods for
//! calculating derived metrics and health checks.

use std::time::Duration;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::WorkerStats;

impl WorkerStats {
    // Note: new() is not needed - Default trait is already implemented in types.rs

    /// Calculate tasks per second
    pub fn tasks_per_second(&self) -> f64 {
        let uptime = self.uptime_seconds.load(Ordering::Relaxed);
        let tasks = self.tasks_processed.load(Ordering::Relaxed);
        
        if uptime > 0 {
            tasks as f64 / uptime as f64
        } else {
            0.0
        }
    }

    /// Calculate average task time in milliseconds
    pub fn avg_task_time_ms(&self) -> f64 {
        self.avg_task_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Check if worker is healthy
    pub fn is_healthy(&self) -> bool {
        // Worker is healthy if:
        // - Task queue isn't too large
        // - Average task time is reasonable
        // - Recent maintenance has occurred

        let queue_ok = self.tasks_queued.load(Ordering::Relaxed) < 1000;
        let task_time_ok = self.avg_task_time_ns.load(Ordering::Relaxed) < 10_000_000; // 10ms
        
        // Check if last maintenance was within 5 minutes
        let last_maintenance_ns = self.last_maintenance_ns.load(Ordering::Relaxed);
        let maintenance_ok = if last_maintenance_ns == 0 {
            true // No maintenance yet is OK at startup
        } else {
            let now_ns = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            let elapsed_ns = now_ns.saturating_sub(last_maintenance_ns);
            elapsed_ns < 300_000_000_000 // 5 minutes in nanoseconds
        };

        queue_ok && task_time_ok && maintenance_ok
    }
}
