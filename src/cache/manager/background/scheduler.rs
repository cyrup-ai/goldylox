//! Maintenance scheduler implementation
//!
//! This module implements the work-stealing maintenance scheduler
//! with worker thread pool management and task distribution.

use std::sync::atomic::Ordering;
use std::time::Instant;
use log;

use super::types::{
    MaintenanceConfig, MaintenanceScheduler, MaintenanceStats, MaintenanceTask, CanonicalMaintenanceTask,
};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey + Default, V: CacheValue + Default> MaintenanceScheduler<K, V> 
where
    K: Clone + bincode::Encode + bincode::Decode<()> + 'static,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static,
{
    /// Create new maintenance scheduler with worker thread pool
    pub fn new(config: MaintenanceConfig) -> Result<Self, CacheOperationError> {
        let (task_sender, task_queue) = if config.queue_capacity > 0 {
            crossbeam_channel::bounded(config.queue_capacity)
        } else {
            crossbeam_channel::unbounded()
        };

        let (shutdown_sender, shutdown_signal) = crossbeam_channel::bounded(1);

        // Create scaling request channels for dynamic worker management
        let (scaling_request_sender, scaling_request_receiver) = crossbeam_channel::bounded(10);
        
        // Initialize global scaling coordination
        super::types::WorkerStatus::initialize_scaling_coordination(scaling_request_sender.clone())
            .map_err(|e| {
                log::warn!("Failed to initialize scaling coordination: {}", e);
                CacheOperationError::InitializationFailed
            })?;

        let _stats = MaintenanceStats::new();

        // Spawn coordinator thread that owns worker management via crossbeam messaging
        let coordinator_handle = {
            let scaling_receiver = scaling_request_receiver.clone();
            let task_queue_clone = task_queue.clone();
            let shutdown_clone = shutdown_signal.clone();
            let config_clone = config.clone();
            
            std::thread::Builder::new()
                .name("maintenance-coordinator".to_string())
                .spawn(move || {
                    Self::coordinator_loop(
                        config_clone,
                        scaling_receiver,
                        task_queue_clone,
                        shutdown_clone
                    );
                })
                .map_err(|_| CacheOperationError::InitializationFailed)?
        };

        log::info!("MaintenanceScheduler coordinator thread spawned with {} initial workers", config.worker_count);

        Ok(Self {
            maintenance_interval_ns: config.heartbeat_interval_ns,
            last_maintenance: Instant::now(),
            maintenance_stats: MaintenanceStats::default(),
            scheduled_operations: Vec::new(),
            task_queue,
            task_sender,
            coordinator_handle: Some(coordinator_handle),
            config,
            stats: MaintenanceStats::default(),
            shutdown_signal,
            shutdown_sender,
            scaling_request_receiver,
            scaling_request_sender,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Submit canonical maintenance task for background processing
    #[inline(always)]
    pub fn submit_task(
        &self,
        canonical_task: CanonicalMaintenanceTask,
        _priority: u16,
    ) -> Result<(), MaintenanceTask> {
        let task = MaintenanceTask::new(canonical_task);

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

    /// Submit urgent canonical maintenance task (bypass queue if full)
    #[inline(always)]
    pub fn submit_urgent_task(&self, canonical_task: CanonicalMaintenanceTask) -> Result<(), String> {
        let task = MaintenanceTask::with_priority(canonical_task, u16::MAX); // Maximum priority

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

    /// Process pending scaling requests by triggering coordinator evaluation
    pub fn process_scaling_requests(&self) -> Result<(), CacheOperationError> {
        // Create a scaling request to trigger coordinator evaluation
        // Use 1.0 capacity factor to maintain current worker count while processing any pending requests
        let (response_sender, response_receiver) = crossbeam_channel::bounded::<Result<(), String>>(1);
        
        let scaling_request = super::types::ScalingRequest {
            capacity_factor: 1.0, // Neutral scaling to trigger processing
            response_sender,
        };
        
        // Send scaling request through crossbeam channel
        self.scaling_request_sender.try_send(scaling_request).map_err(|e| {
            CacheOperationError::ResourceExhausted(format!("Failed to send scaling request: {:?}", e))
        })?;
        
        // Wait for coordinator response with timeout
        match response_receiver.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(result) => result.map_err(|e| CacheOperationError::resource_exhausted(&e)),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                Err(CacheOperationError::resource_exhausted("Scaling request timed out"))
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                Err(CacheOperationError::InternalError)
            }
        }
    }

    /// Coordinator thread loop that owns worker threads and processes scaling requests
    fn coordinator_loop(
        mut config: MaintenanceConfig,
        scaling_receiver: crossbeam_channel::Receiver<super::types::ScalingRequest>,
        task_queue: crossbeam_channel::Receiver<MaintenanceTask>,
        shutdown_signal: crossbeam_channel::Receiver<()>,
    ) {
        use std::time::Duration;
        let mut worker_threads: Vec<(std::thread::JoinHandle<()>, crossbeam_channel::Sender<()>)> = Vec::with_capacity(config.worker_count as usize);
        
        // Spawn initial worker threads
        for worker_id in 0..config.worker_count {
            let task_receiver = task_queue.clone();
            let global_shutdown_receiver = shutdown_signal.clone();
            let config_clone = config.clone();
            
            // Create individual shutdown channel for this worker
            let (worker_shutdown_sender, _worker_shutdown_receiver) = crossbeam_channel::bounded(1);

            match std::thread::Builder::new()
                .name(format!("maintenance-worker-{}", worker_id))
                .spawn(move || {
                    Self::worker_loop(
                        worker_id,
                        task_receiver,
                        global_shutdown_receiver,
                        &crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS,
                        config_clone,
                    );
                }) {
                Ok(handle) => worker_threads.push((handle, worker_shutdown_sender)),
                Err(e) => log::error!("Failed to spawn initial worker {}: {:?}", worker_id, e),
            }
        }
        
        log::info!("Coordinator started with {} workers", worker_threads.len());
        
        // Main coordinator loop
        loop {
            // Process scaling requests with owned data
            while let Ok(scaling_request) = scaling_receiver.try_recv() {
                let result = Self::handle_scaling_with_ownership(
                    &mut worker_threads,
                    &mut config,
                    &scaling_request,
                    &task_queue,
                    &shutdown_signal
                );
                let _ = scaling_request.response_sender.try_send(result);
            }
            
            // Check for shutdown signal
            if shutdown_signal.try_recv().is_ok() {
                log::info!("Coordinator shutting down, joining {} worker threads", worker_threads.len());
                
                // Signal all workers to shutdown first
                for (_, worker_shutdown_sender) in &worker_threads {
                    let _ = worker_shutdown_sender.try_send(());
                }
                
                // Then join all worker threads
                for (handle, _) in worker_threads.drain(..) {
                    if let Err(e) = handle.join() {
                        log::warn!("Worker thread panicked during shutdown: {:?}", e);
                    }
                }
                break;
            }
            
            // Brief sleep to avoid busy waiting
            std::thread::sleep(Duration::from_millis(10));
        }
        
        log::info!("Coordinator thread exiting");
    }

    /// Handle scaling request with owned worker threads and config
    fn handle_scaling_with_ownership(
        worker_threads: &mut Vec<(std::thread::JoinHandle<()>, crossbeam_channel::Sender<()>)>,
        config: &mut MaintenanceConfig,
        request: &super::types::ScalingRequest,
        task_queue: &crossbeam_channel::Receiver<MaintenanceTask>,
        shutdown_signal: &crossbeam_channel::Receiver<()>,
    ) -> Result<(), String> {
        let current_worker_count = config.worker_count;
        let target_worker_count = ((current_worker_count as f64) * request.capacity_factor).round() as u32;
        
        // Validate target worker count
        if target_worker_count == 0 {
            return Err("Cannot scale to zero workers".to_string());
        }
        
        if target_worker_count > 100 {
            return Err("Cannot scale beyond 100 workers".to_string());
        }
        
        if target_worker_count == current_worker_count {
            log::debug!("No scaling needed: already at target worker count {}", target_worker_count);
            return Ok(());
        }
        
        log::info!("Scaling workers from {} to {} (factor: {})", 
                   current_worker_count, target_worker_count, request.capacity_factor);
        
        if target_worker_count > current_worker_count {
            // Scale up: spawn additional worker threads
            for worker_id in current_worker_count..target_worker_count {
                let task_receiver = task_queue.clone();
                let global_shutdown_receiver = shutdown_signal.clone();
                let config_clone = config.clone();
                
                // Create individual shutdown channel for new worker
                let (worker_shutdown_sender, _worker_shutdown_receiver) = crossbeam_channel::bounded(1);
                
                match std::thread::Builder::new()
                    .name(format!("maintenance-worker-{}", worker_id))
                    .spawn(move || {
                        Self::worker_loop(
                            worker_id,
                            task_receiver,
                            global_shutdown_receiver,
                            &crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS,
                            config_clone,
                        );
                    }) {
                    Ok(handle) => worker_threads.push((handle, worker_shutdown_sender)),
                    Err(_) => return Err("Failed to spawn worker thread".to_string()),
                }
            }
        } else {
            // Scale down: gracefully shutdown excess worker threads
            let threads_to_remove = current_worker_count - target_worker_count;
            for _ in 0..threads_to_remove {
                if let Some((handle, worker_shutdown_sender)) = worker_threads.pop() {
                    // Send shutdown signal to specific worker first
                    if let Err(e) = worker_shutdown_sender.send(()) {
                        log::warn!("Failed to send shutdown signal to worker: {:?}", e);
                    }
                    
                    // Now safe to join the worker thread
                    if let Err(e) = handle.join() {
                        log::warn!("Worker thread panicked during scale-down: {:?}", e);
                    }
                }
            }
        }
        
        // Update worker count in config
        config.worker_count = target_worker_count;
        log::info!("Worker scaling completed: now have {} workers", target_worker_count);
        Ok(())
    }

    /// Shutdown maintenance scheduler
    pub fn shutdown(mut self) -> Result<(), CacheOperationError> {
        // Signal shutdown to coordinator (which will shutdown all workers)
        let _ = self.shutdown_sender.send(());

        // Wait for coordinator thread to complete
        if let Some(coordinator_handle) = self.coordinator_handle.take() {
            if let Err(e) = coordinator_handle.join() {
                log::error!("Coordinator thread panicked: {:?}", e);
            }
        }

        Ok(())
    }
}
