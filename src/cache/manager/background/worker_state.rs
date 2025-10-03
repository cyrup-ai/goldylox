//! Worker state management
//!
//! This module implements worker state tracking and health monitoring
//! for background maintenance workers.

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use crossbeam_utils::atomic::AtomicCell;

use super::types::{BackgroundWorkerState, WorkerStatus};

/// Channel-based worker status communication
#[derive(Debug)]
pub struct WorkerStatusChannel {
    /// Channel receiver for worker status updates
    status_receiver: Receiver<WorkerStatusSnapshot>,
    /// Cached latest status (updated on demand)
    cached_status: std::sync::RwLock<Option<WorkerStatusSnapshot>>,
    /// Last update timestamp
    last_update: std::sync::atomic::AtomicU64, // nanoseconds since epoch
}

/// Serializable snapshot of worker status for channel transmission
#[derive(Debug, Clone)]
pub struct WorkerStatusSnapshot {
    pub worker_id: u32,
    pub tasks_processed: u64,
    pub total_processing_time: u64,
    pub status: WorkerStatus,
    pub last_heartbeat_ns: u64, // nanoseconds since epoch
    pub current_task_discriminant: u8,
    pub error_count: u32,
    pub steal_attempts: u64,
    pub successful_steals: u64,
    pub tasks_since_heartbeat: u64,
    pub is_healthy: bool,
    pub snapshot_time_ns: u64, // when this snapshot was created
}

impl WorkerStatusChannel {
    /// Get latest worker status, updating cache if new data available
    pub fn get_latest_status(&self) -> Option<WorkerStatusSnapshot> {
        // Try to receive latest status updates (non-blocking)
        while let Ok(status) = self.status_receiver.try_recv() {
            // Update cached status
            if let Ok(mut cached) = self.cached_status.write() {
                *cached = Some(status.clone());
                self.last_update.store(
                    status.snapshot_time_ns,
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
        }

        // Return cached status
        self.cached_status.read().ok()?.clone()
    }

    /// Check if status is recent (within timeout)
    pub fn is_status_recent(&self, timeout_ns: u64) -> bool {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let last_update = self.last_update.load(std::sync::atomic::Ordering::Relaxed);
        now_ns - last_update < timeout_ns
    }
}

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
    pub fn shutdown_workers<
        K: crate::cache::traits::CacheKey + Default + bincode::Encode + bincode::Decode<()>,
        V: crate::cache::traits::CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        scheduler: &crate::cache::manager::background::types::MaintenanceScheduler<K, V>,
    ) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
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
        self.error_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.status.store(WorkerStatus::Error);
        warn!(
            "Worker {} encountered error: {}",
            self.worker_id, error_message
        );
    }

    /// Check if worker is in error state and attempt recovery
    pub fn attempt_recovery(&self) -> bool {
        if matches!(self.status(), WorkerStatus::Error) {
            let error_count = self.error_count.load(std::sync::atomic::Ordering::Relaxed);
            if error_count < 5 {
                // Allow up to 5 errors before permanent failure
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
        config.work_stealing_active
            && matches!(
                self.status(),
                WorkerStatus::Idle | WorkerStatus::StealingWork
            )
    }

    /// Create status snapshot for channel transmission
    pub fn create_status_snapshot(&self) -> WorkerStatusSnapshot {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        WorkerStatusSnapshot {
            worker_id: self.worker_id,
            tasks_processed: self
                .tasks_processed
                .load(std::sync::atomic::Ordering::Relaxed),
            total_processing_time: self
                .total_processing_time
                .load(std::sync::atomic::Ordering::Relaxed),
            status: self.status.load(),
            last_heartbeat_ns: self.last_heartbeat.load().elapsed().as_nanos() as u64,
            current_task_discriminant: self
                .current_task_discriminant
                .load(std::sync::atomic::Ordering::Relaxed),
            error_count: self.error_count.load(std::sync::atomic::Ordering::Relaxed),
            steal_attempts: self
                .steal_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            successful_steals: self
                .successful_steals
                .load(std::sync::atomic::Ordering::Relaxed),
            tasks_since_heartbeat: self
                .tasks_since_heartbeat
                .load(std::sync::atomic::Ordering::Relaxed),
            is_healthy: self.is_healthy(),
            snapshot_time_ns: now_ns,
        }
    }

    /// Register worker status channel in per-instance registry for monitoring
    pub fn register_worker_channel(
        registry: &std::sync::Arc<dashmap::DashMap<u32, WorkerStatusChannel>>,
        worker_id: u32,
        status_receiver: Receiver<WorkerStatusSnapshot>,
    ) {
        let channel = WorkerStatusChannel {
            status_receiver,
            cached_status: std::sync::RwLock::new(None),
            last_update: std::sync::atomic::AtomicU64::new(0),
        };
        registry.insert(worker_id, channel);
    }

    /// Unregister worker state from per-instance registry
    pub fn unregister_worker(
        registry: &std::sync::Arc<dashmap::DashMap<u32, WorkerStatusChannel>>,
        worker_id: u32,
    ) {
        registry.remove(&worker_id);
    }

    /// Get all registered worker task counts via channel communication
    pub fn get_all_worker_task_counts(
        registry: &std::sync::Arc<dashmap::DashMap<u32, WorkerStatusChannel>>,
    ) -> Vec<(u32, u64)> {
        registry
            .iter()
            .filter_map(|entry| {
                let worker_id = *entry.key();
                let channel = entry.value();
                channel
                    .get_latest_status()
                    .map(|status| (worker_id, status.tasks_processed))
            })
            .collect()
    }

    /// Get all registered worker health statuses via channel communication
    pub fn get_all_worker_health(
        registry: &std::sync::Arc<dashmap::DashMap<u32, WorkerStatusChannel>>,
    ) -> Vec<(u32, bool)> {
        registry
            .iter()
            .filter_map(|entry| {
                let worker_id = *entry.key();
                let channel = entry.value();
                channel
                    .get_latest_status()
                    .map(|status| (worker_id, status.is_healthy))
            })
            .collect()
    }
}
