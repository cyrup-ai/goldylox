//! Worker state management
//!
//! This module implements worker state tracking and health monitoring
//! for background maintenance workers.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_utils::atomic::AtomicCell;

use super::types::{BackgroundWorkerState, WorkerStatus};

impl BackgroundWorkerState {
    /// Create new background worker state
    pub fn new(worker_id: u32) -> Self {
        Self {
            worker_id,
            tasks_processed: AtomicU64::new(0),
            total_processing_time: AtomicU64::new(0),
            status: AtomicCell::new(WorkerStatus::Idle),
            last_heartbeat: AtomicCell::new(Instant::now()),
            current_task: AtomicCell::new(None),
            error_count: AtomicU32::new(0),
            steal_attempts: AtomicU64::new(0),
            successful_steals: AtomicU64::new(0),
            tasks_since_heartbeat: AtomicU64::new(0),
        }
    }

    /// Get worker ID
    #[inline(always)]
    pub fn worker_id(&self) -> u32 {
        self.worker_id
    }

    /// Get tasks processed count
    #[inline(always)]
    pub fn tasks_processed(&self) -> u64 {
        self.tasks_processed.load(Ordering::Relaxed)
    }

    /// Get current worker status
    #[inline(always)]
    pub fn status(&self) -> WorkerStatus {
        self.status.load()
    }

    /// Check if worker is healthy (recent heartbeat)
    #[inline(always)]
    pub fn is_healthy(&self) -> bool {
        let last_heartbeat = self.last_heartbeat.load();
        let now = Instant::now();
        now.duration_since(last_heartbeat) < Duration::from_secs(30)
    }

    /// Shutdown workers (placeholder for comprehensive shutdown)
    #[inline(always)]
    pub fn shutdown_workers(&self, scheduler: &crate::cache::manager::MaintenanceScheduler) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        // Update local status to shutdown
        self.status.store(WorkerStatus::Shutdown);
        
        // Coordinate with the actual scheduler that owns worker threads
        scheduler.stop_maintenance()
    }
}
