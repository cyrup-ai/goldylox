//! Cache maintenance worker management
//!
//! This module implements the core CacheMaintenanceWorker with lifecycle
//! management, task submission, and worker thread spawning.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam::channel::{unbounded, Receiver};

use super::types::{CacheMaintenanceWorker, MaintenanceTask, WorkerStats};
use crate::cache::traits::types_and_enums::CacheOperationError;

impl CacheMaintenanceWorker {
    /// Create new maintenance worker
    pub fn new() -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (task_sender, task_receiver) = unbounded();
        let stats = Arc::new(Mutex::new(WorkerStats::new()));

        let worker_handle = Some(Self::spawn_worker(
            shutdown.clone(),
            task_receiver,
            stats.clone(),
        ));

        Self {
            worker_handle,
            shutdown,
            task_sender,
            stats,
        }
    }

    /// Start the maintenance worker
    pub fn start(&mut self) -> Result<(), CacheOperationError> {
        if self.worker_handle.is_none() {
            let (task_sender, task_receiver) = unbounded();
            self.task_sender = task_sender;
            self.shutdown.store(false, Ordering::Relaxed);

            let worker_handle =
                Self::spawn_worker(self.shutdown.clone(), task_receiver, self.stats.clone());

            self.worker_handle = Some(worker_handle);
        }
        Ok(())
    }

    /// Stop the maintenance worker
    pub fn stop(&mut self) -> Result<(), CacheOperationError> {
        if let Some(handle) = self.worker_handle.take() {
            self.shutdown.store(true, Ordering::Relaxed);

            // Send shutdown signal through channel
            let _ = self.task_sender.send(MaintenanceTask::UpdateStatistics);

            // Wait for worker to finish
            if let Err(_) = handle.join() {
                return Err(CacheOperationError::resource_exhausted(
                    "Failed to join worker thread",
                ));
            }
        }
        Ok(())
    }

    /// Submit maintenance task
    pub fn submit_task(&self, task: MaintenanceTask) -> Result<(), CacheOperationError> {
        self.task_sender.send(task).map_err(|_| {
            CacheOperationError::resource_exhausted("Failed to send task to worker")
        })?;

        // Update queued task count
        if let Ok(mut stats) = self.stats.lock() {
            stats.tasks_queued += 1;
        }

        Ok(())
    }

    /// Get worker statistics
    pub fn stats(&self) -> Result<WorkerStats, CacheOperationError> {
        let stats = self.stats.lock().map_err(|_| {
            CacheOperationError::resource_exhausted("Worker stats lock poisoned")
        })?;
        Ok(stats.clone())
    }

    /// Schedule automatic maintenance
    pub fn schedule_maintenance(&self) -> Result<(), CacheOperationError> {
        self.submit_task(MaintenanceTask::CleanupExpired)?;
        self.submit_task(MaintenanceTask::OptimizeLayout)?;
        self.submit_task(MaintenanceTask::UpdateStatistics)?;
        Ok(())
    }

    /// Spawn worker thread
    pub fn spawn_worker(
        shutdown: Arc<AtomicBool>,
        task_receiver: Receiver<MaintenanceTask>,
        stats: Arc<Mutex<WorkerStats>>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let start_time = Instant::now();
            let mut last_maintenance = None;

            while !shutdown.load(Ordering::Relaxed) {
                // Try to receive task with timeout
                match task_receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(task) => {
                        let task_start = Instant::now();
                        super::task_processor::process_task(task, &stats);
                        let task_duration = task_start.elapsed();

                        // Update statistics
                        if let Ok(mut worker_stats) = stats.lock() {
                            worker_stats.tasks_processed += 1;
                            worker_stats.tasks_queued = worker_stats.tasks_queued.saturating_sub(1);
                            worker_stats.uptime_seconds = start_time.elapsed().as_secs();

                            // Update average task time
                            let current_avg = worker_stats.avg_task_time_ns;
                            let new_time = task_duration.as_nanos() as u64;
                            worker_stats.avg_task_time_ns = if current_avg == 0 {
                                new_time
                            } else {
                                (current_avg * 3 + new_time) / 4 // Exponential moving average
                            };
                        }
                    }
                    Err(_) => {
                        // Timeout - perform periodic maintenance
                        if last_maintenance
                            .map_or(true, |t: Instant| t.elapsed() > Duration::from_secs(60))
                        {
                            super::task_processor::perform_periodic_maintenance(&stats);
                            last_maintenance = Some(Instant::now());
                        }
                    }
                }
            }

            // Final statistics update
            if let Ok(mut worker_stats) = stats.lock() {
                worker_stats.uptime_seconds = start_time.elapsed().as_secs();
            }
        })
    }
}

impl Drop for CacheMaintenanceWorker {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl Default for CacheMaintenanceWorker {
    fn default() -> Self {
        Self::new()
    }
}
