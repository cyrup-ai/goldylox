//! Cache maintenance worker management
//!
//! This module implements the core CacheMaintenanceWorker with lifecycle
//! management, task submission, and worker thread spawning.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossbeam::channel::{unbounded, Receiver, Sender};

use super::types::{CacheMaintenanceWorker, MaintenanceTask, StatUpdate, WorkerMaintenanceOps};
use crate::cache::traits::types_and_enums::CacheOperationError;

impl CacheMaintenanceWorker {
    // Create new maintenance worker - already defined in types.rs

    /// Start the maintenance worker
    pub fn start(&mut self) -> Result<(), CacheOperationError> {
        if self.worker_handle.is_none() {
            // Take the task_receiver - it can only be used once
            let task_receiver = self.task_receiver.take()
                .ok_or_else(|| CacheOperationError::resource_exhausted(
                    "Worker already started - task_receiver already taken"
                ))?;
            
            // Clone stat_sender for the worker thread
            let stat_sender = self.stat_sender.clone();
            
            self.shutdown.store(false, Ordering::Relaxed);

            let worker_handle =
                Self::spawn_worker(
                    &self.shutdown as *const AtomicBool,
                    task_receiver,
                    stat_sender,
                );

            self.worker_handle = Some(worker_handle);
        }
        Ok(())
    }

    /// Stop the maintenance worker
    pub fn stop(&mut self) -> Result<(), CacheOperationError> {
        if let Some(handle) = self.worker_handle.take() {
            self.shutdown.store(true, Ordering::Relaxed);

            // Send shutdown signal through channel
            let _ = self.task_sender.send(WorkerMaintenanceOps::update_statistics_task());

            // Wait for worker to finish
            if let Err(_) = handle.join() {
                return Err(CacheOperationError::resource_exhausted(
                    "Failed to join worker thread",
                ));
            }
        }
        Ok(())
    }

    // Submit maintenance task - already defined in types.rs
    // Get worker statistics - already defined in types.rs as stats(&mut self)

    /// Schedule automatic maintenance
    pub fn schedule_maintenance(&self) -> Result<(), CacheOperationError> {
        self.submit_task(WorkerMaintenanceOps::cleanup_expired_task())?;
        self.submit_task(WorkerMaintenanceOps::optimize_layout_task())?;
        self.submit_task(WorkerMaintenanceOps::update_statistics_task())?;
        Ok(())
    }

    /// Spawn worker thread
    pub fn spawn_worker(
        shutdown_ptr: *const AtomicBool,
        task_receiver: Receiver<MaintenanceTask>,
        stat_sender: Sender<StatUpdate>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            // SAFETY: The shutdown flag lives as long as the CacheMaintenanceWorker
            // and we only read from it. The worker thread is joined before Drop.
            let shutdown = unsafe { &*shutdown_ptr };
            
            let start_time = Instant::now();

            while !shutdown.load(Ordering::Relaxed) {
                // Try to receive task with timeout
                match task_receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(task) => {
                        let task_start = Instant::now();
                        super::task_processor::process_task(task, &stat_sender);
                        let task_duration = task_start.elapsed();

                        // Send statistics updates through channel
                        let _ = stat_sender.send(StatUpdate::TaskProcessed);
                        let _ = stat_sender.send(StatUpdate::TaskTime(
                            task_duration.as_nanos() as u64
                        ));
                        let _ = stat_sender.send(StatUpdate::UpdateUptime(
                            start_time.elapsed().as_secs()
                        ));
                    }
                    Err(_) => {
                        // Timeout - perform periodic maintenance
                        super::task_processor::perform_periodic_maintenance(&stat_sender);
                        
                        // Update uptime periodically
                        let _ = stat_sender.send(StatUpdate::UpdateUptime(
                            start_time.elapsed().as_secs()
                        ));
                    }
                }
            }

            // Final statistics update
            let _ = stat_sender.send(StatUpdate::UpdateUptime(
                start_time.elapsed().as_secs()
            ));
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
