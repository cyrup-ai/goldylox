//! Worker state management
//!
//! This module implements worker state tracking and health monitoring
//! for background maintenance workers.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
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
            current_task_discriminant: AtomicU8::new(0),
            error_count: AtomicU32::new(0),
            steal_attempts: AtomicU64::new(0),
            successful_steals: AtomicU64::new(0),
            tasks_since_heartbeat: AtomicU64::new(0),
        }
    }

    /// Get worker ID
    #[allow(dead_code)] // Background workers - worker_id used in worker identification and coordination
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

    /// Shutdown workers
    #[inline(always)]
    pub fn shutdown_workers<K: crate::cache::traits::CacheKey + Default + bincode::Encode + bincode::Decode<()>, V: crate::cache::traits::CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(&self, scheduler: &crate::cache::manager::background::types::MaintenanceScheduler<K, V>) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        // Update local status to shutdown
        self.status.store(WorkerStatus::Shutdown);
        
        // Coordinate with the actual scheduler that owns worker threads
        scheduler.stop_maintenance()
    }
    
    /// Start work stealing process if enabled in configuration
    pub fn start_work_stealing(&self, config: &super::types::MaintenanceConfig) -> bool {
        if config.work_stealing_active {
            self.status.store(WorkerStatus::StealingWork);
            true
        } else {
            false
        }
    }
    
    /// Record an error and update worker status
    pub fn record_error(&self, error_message: &str) {
        use log::warn;
        self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.status.store(WorkerStatus::Error);
        warn!("Worker {} encountered error: {}", self.worker_id, error_message);
    }
    
    /// Check if worker is in error state and attempt recovery
    pub fn attempt_recovery(&self) -> bool {
        if matches!(self.status(), WorkerStatus::Error) {
            let error_count = self.error_count.load(std::sync::atomic::Ordering::Relaxed);
            if error_count < 5 { // Allow up to 5 errors before permanent failure
                // Reset to idle to attempt recovery
                self.status.store(WorkerStatus::Idle);
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    
    /// Check if worker can steal work based on current status and configuration
    pub fn can_steal_work(&self, config: &super::types::MaintenanceConfig) -> bool {
        config.work_stealing_active && matches!(self.status(), WorkerStatus::Idle | WorkerStatus::StealingWork)
    }
}
