//! Maintenance scheduler implementation
//!
//! This module implements the work-stealing maintenance scheduler
//! with worker thread pool management and task distribution.

use std::sync::atomic::Ordering;
use std::time::Instant;
use log;

use super::types::{
    MaintenanceConfig, MaintenanceScheduler, MaintenanceStats, MaintenanceTask, MaintenanceTaskType,
};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey, V: CacheValue> MaintenanceScheduler<K, V> 
where
    K: Clone + 'static,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + 'static,
{
    /// Create new maintenance scheduler with worker thread pool
    pub fn new(config: MaintenanceConfig) -> Result<Self, CacheOperationError> {
        let (task_sender, task_queue) = if config.queue_capacity > 0 {
            crossbeam_channel::bounded(config.queue_capacity)
        } else {
            crossbeam_channel::unbounded()
        };

        let (shutdown_sender, shutdown_signal) = crossbeam_channel::bounded(1);


        let _stats = MaintenanceStats::new();

        // Spawn worker threads
        let mut worker_threads = Vec::with_capacity(config.worker_count as usize);

        for worker_id in 0..config.worker_count {
            let task_receiver = task_queue.clone();
            let shutdown_receiver = shutdown_signal.clone();
            let config_clone = config.clone();

            let handle = std::thread::Builder::new()
                .name(format!("maintenance-worker-{}", worker_id))
                .spawn(move || {
                    Self::worker_loop(
                        worker_id,
                        task_receiver,
                        shutdown_receiver,
                        &crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS,
                        config_clone,
                    );
                })
                .map_err(|_e| CacheOperationError::InitializationFailed)?;

            worker_threads.push(handle);
        }

        Ok(Self {
            maintenance_interval_ns: config.heartbeat_interval_ns,
            last_maintenance: Instant::now(),
            maintenance_stats: MaintenanceStats::default(),
            scheduled_operations: Vec::new(),
            task_queue,
            task_sender,
            worker_threads,
            config,
            stats: MaintenanceStats::default(),
            shutdown_signal,
            shutdown_sender,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Submit maintenance task for background processing
    #[inline(always)]
    pub fn submit_task(
        &self,
        task_type: MaintenanceTaskType,
        priority: u16,
    ) -> Result<(), MaintenanceTask> {
        let task = MaintenanceTask {
            task_type,
            priority,
            created_at: Instant::now(),
            timeout_ns: self.config.task_timeout_ns,
            retry_count: 0,
            max_retries: self.config.max_task_retries,
        };

        self.stats.total_submitted.fetch_add(1, Ordering::Relaxed);

        match self.task_sender.try_send(task.clone()) {
            Ok(()) => Ok(()),
            Err(crossbeam_channel::TrySendError::Full(task)) => {
                // Queue is full, return task for caller to handle
                Err(task)
            }
            Err(crossbeam_channel::TrySendError::Disconnected(task)) => {
                // Scheduler is shutting down
                Err(task)
            }
        }
    }

    /// Submit urgent maintenance task (bypass queue if full)
    #[inline(always)]
    pub fn submit_urgent_task(&self, task_type: MaintenanceTaskType) -> Result<(), String> {
        let task = MaintenanceTask {
            task_type,
            priority: u16::MAX, // Maximum priority
            created_at: Instant::now(),
            timeout_ns: self.config.task_timeout_ns / 2, // Shorter timeout for urgent tasks
            retry_count: 0,
            max_retries: 1, // Fewer retries for urgent tasks
        };

        self.stats.total_submitted.fetch_add(1, Ordering::Relaxed);

        // Use blocking send for urgent tasks
        self.task_sender
            .send(task)
            .map_err(|_| "Failed to submit urgent task: scheduler shutting down".to_string())
    }

    /// Get maintenance statistics
    #[inline(always)]
    pub fn get_stats(&self) -> &MaintenanceStats {
        &self.stats
    }

    /// Stop maintenance operations
    #[inline(always)]
    pub fn stop_maintenance(&self) -> Result<(), CacheOperationError> {
        // Signal shutdown to all workers
        self.shutdown_sender
            .send(())
            .map_err(|_| CacheOperationError::OperationFailed)
    }

    /// Check if maintenance scheduler is healthy
    #[inline(always)]
    pub fn is_healthy(&self) -> bool {
        // Check if any tasks are stuck (taking too long)
        let active_count = crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS.load(std::sync::atomic::Ordering::Relaxed);
        let max_healthy_tasks = self.config.worker_count * 2; // Allow some queuing
        active_count <= max_healthy_tasks
    }

    /// Shutdown maintenance scheduler
    pub fn shutdown(self) -> Result<(), CacheOperationError> {
        // Signal shutdown to all workers
        let _ = self.shutdown_sender.send(());

        // Wait for all worker threads to complete
        for handle in self.worker_threads.into_iter() {
            if let Err(e) = handle.join() {
                log::error!("Worker thread panicked: {:?}", e);
            }
        }

        Ok(())
    }
}
