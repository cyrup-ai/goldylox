//! Cache maintenance worker management
//!
//! This module implements the core CacheMaintenanceWorker with lifecycle
//! management, task submission, and worker thread spawning.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam::channel::Sender;
use tokio::sync::mpsc;

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
                    self.cold_tier_coordinator.clone(),
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

    /// Spawn async worker task
    pub fn spawn_worker(
        shutdown_ptr: *const AtomicBool,
        task_receiver: mpsc::UnboundedReceiver<MaintenanceTask>,
        stat_sender: Sender<StatUpdate>,
        cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // SAFETY: The shutdown flag lives as long as the CacheMaintenanceWorker
            // and we only read from it. The worker thread is joined before Drop.
            let shutdown = unsafe { &*shutdown_ptr };
            
            let start_time = Instant::now();
            let mut task_receiver = task_receiver;  // Make mutable for async recv
            
            // Periodic maintenance interval (100ms to match previous behavior)
            let mut maintenance_interval = tokio::time::interval(Duration::from_millis(100));
            maintenance_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    // Priority 1: Process incoming tasks instantly
                    Some(task) = task_receiver.recv() => {
                        let task_start = Instant::now();
                        
                        // Pass cold_tier_coordinator to fix signature
                        super::task_processor::process_task(
                            task, 
                            &stat_sender,
                            &cold_tier_coordinator
                        );
                        
                        let task_duration = task_start.elapsed();

                        // Send statistics updates
                        let _ = stat_sender.send(StatUpdate::TaskProcessed);
                        let _ = stat_sender.send(StatUpdate::TaskTime(
                            task_duration.as_nanos() as u64
                        ));
                        let _ = stat_sender.send(StatUpdate::UpdateUptime(
                            start_time.elapsed().as_secs()
                        ));
                        
                        // Check shutdown after processing
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    
                    // Priority 2: Periodic maintenance every 100ms when idle
                    _ = maintenance_interval.tick() => {
                        // Only run if not shutting down
                        if !shutdown.load(Ordering::Relaxed) {
                            super::task_processor::perform_periodic_maintenance(
                                &stat_sender,
                                &cold_tier_coordinator
                            );
                            
                            // Update uptime periodically
                            let _ = stat_sender.send(StatUpdate::UpdateUptime(
                                start_time.elapsed().as_secs()
                            ));
                        }
                    }
                }
                
                // Break if shutdown flag is set
                if shutdown.load(Ordering::Relaxed) {
                    break;
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

// Note: Default trait removed because new() now requires cold_tier_coordinator parameter
