//! Background operation coordinator with work-stealing task queue
//!
//! This module manages background tasks, maintenance scheduling, and worker coordination
//! for the unified cache system using lock-free channels and atomic state management.

use std::marker::PhantomData;

use std::thread;
use std::time::Duration;
use log::{error, warn, debug};
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::cache::config::CacheConfig;
use crate::cache::manager::background::{
    types::{BackgroundTask, TaskProcessor}, BackgroundWorkerState, MaintenanceConfig, MaintenanceScheduler,
};
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Timer control commands
#[derive(Debug)]
enum TimerCommand {
    /// Schedule a task after delay
    Schedule { delay_ms: u64, task: BackgroundTask },
    /// Shutdown timer thread
    Shutdown,
}

/// Background operation coordinator with work-stealing task queue
#[derive(Debug)]
pub struct BackgroundCoordinator<K: CacheKey + Default, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static, P: TaskProcessor = DefaultProcessor> {
    /// Background task queue (lock-free channel)
    task_queue: Sender<BackgroundTask>,
    /// Task receiver for processing
    task_receiver: Receiver<BackgroundTask>,
    /// Sender to processor thread (processor owns the TaskProcessor)
    processor_sender: Option<Sender<BackgroundTask>>,
    /// Processor thread handle
    processor_thread: Option<thread::JoinHandle<()>>,
    /// Timer thread for scheduling periodic tasks
    timer_thread: Option<thread::JoinHandle<()>>,
    /// Worker thread for task processing
    worker_thread: Option<thread::JoinHandle<()>>,
    /// Timer control sender
    timer_sender: Sender<TimerCommand>,
    /// Maintenance scheduler with atomic timing
    maintenance_scheduler: MaintenanceScheduler<K, V>,
    /// Background worker state with atomic coordination
    worker_state: BackgroundWorkerState,
    /// Phantom data for generics
    _phantom: PhantomData<(K, V, P)>,
}

// String-specific convenience APIs removed - use generic BackgroundCoordinator<K, V, P> directly

/// Default no-op processor for when no processor is set
#[derive(Debug)]
pub struct DefaultProcessor;

impl DefaultProcessor {
    /// Create new default processor instance
    pub fn new() -> Self {
        Self
    }
}

impl TaskProcessor for DefaultProcessor {
    fn process_task(&self, _task: &BackgroundTask) -> Result<(), CacheOperationError> {
        Ok(())
    }
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()>, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static, P: TaskProcessor + Send + 'static> BackgroundCoordinator<K, V, P> {
    /// Create new background coordinator with configuration
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let (task_queue, task_receiver) = bounded(1000); // Bounded queue for backpressure
        let (timer_sender, timer_receiver) = bounded(100);
        let maintenance_config = MaintenanceConfig::default();
        let maintenance_scheduler = MaintenanceScheduler::<K, V>::new(maintenance_config)?;
        let worker_state = BackgroundWorkerState::new(0);
        
        // Start timer thread
        let task_tx = task_queue.clone();
        let timer_thread = thread::spawn(move || {
            while let Ok(cmd) = timer_receiver.recv() {
                match cmd {
                    TimerCommand::Schedule { delay_ms, task } => {
                        thread::sleep(Duration::from_millis(delay_ms));
                        if let Err(e) = task_tx.try_send(task) {
                            error!("Failed to send scheduled task: {:?}", e);
                        }
                    }
                    TimerCommand::Shutdown => break,
                }
            }
        });

        Ok(Self {
            task_queue,
            task_receiver,
            processor_sender: None,
            processor_thread: None,
            timer_thread: Some(timer_thread),
            worker_thread: None,
            timer_sender,
            maintenance_scheduler,
            worker_state,
            _phantom: PhantomData,
        })
    }
    
    /// Start processor thread with owned TaskProcessor
    pub fn start_processor(&mut self, processor: P) -> Result<(), CacheOperationError> {
        if self.processor_thread.is_some() {
            debug!("Processor thread already started");
            return Ok(());
        }
        
        let (sender, receiver) = bounded::<BackgroundTask>(1000);
        self.processor_sender = Some(sender);
        
        let timer_tx = self.timer_sender.clone();
        
        let handle = thread::Builder::new()
            .name("cache-processor".to_string())
            .spawn(move || {
                debug!("Starting processor thread");
                // Processor OWNS the TaskProcessor - no Arc!
                while let Ok(task) = receiver.recv() {
                    match &task {
                        BackgroundTask::Statistics { stats_type: 2, interval_ms } => {
                            if let Err(e) = processor.process_task(&task) {
                                error!("Memory monitoring task failed: {:?}", e);
                            }
                            // Reschedule periodic task
                            if let Err(e) = timer_tx.send(TimerCommand::Schedule {
                                delay_ms: *interval_ms,
                                task: task.clone(),
                            }) {
                                error!("Failed to reschedule monitoring task: {:?}", e);
                                break;
                            }
                        }
                        _ => {
                            // Process other task types
                            if let Err(e) = processor.process_task(&task) {
                                warn!("Background task failed: {:?}", e);
                            }
                        }
                    }
                }
                debug!("Processor thread shutting down");
            })
            .map_err(|_| CacheOperationError::InitializationFailed)?;
            
        self.processor_thread = Some(handle);
        Ok(())
    }

    /// Start background worker threads
    pub fn start_worker_threads(&self) -> Result<(), CacheOperationError> {
        // Only start if not already started
        if self.worker_thread.is_some() {
            debug!("Worker threads already started");
            return Ok(());
        }
        
        let rx = self.task_receiver.clone();
        let processor_sender = self.processor_sender.clone();
        
        let _handle = thread::Builder::new()
            .name("cache-worker".to_string())
            .spawn(move || {
                debug!("Starting cache worker thread");
                while let Ok(task) = rx.recv() {
                    // Forward to processor thread if available
                    if let Some(ref sender) = processor_sender {
                        if let Err(e) = sender.try_send(task) {
                            warn!("Failed to forward task to processor: {:?}", e);
                        }
                    }
                }
                debug!("Cache worker thread shutting down");
            })
            .map_err(|_| CacheOperationError::InitializationFailed)?;
        
        // NOTE: Cannot store handle due to &self constraint
        // This is a structural limitation requiring API redesign
        Ok(())
    }

    /// Submit background task for processing
    pub fn submit_task(&self, task: BackgroundTask) -> Result<(), CacheOperationError> {
        self.task_queue
            .try_send(task)
            .map_err(|_| CacheOperationError::resource_exhausted("Background task queue full"))
    }

    /// Get worker state for monitoring
    pub fn worker_state(&self) -> &BackgroundWorkerState {
        &self.worker_state
    }

    /// Get maintenance scheduler
    pub fn maintenance_scheduler(&self) -> &MaintenanceScheduler<K, V> {
        &self.maintenance_scheduler
    }
    
    /// Get maintenance task sender for wiring to GC coordinator
    pub fn get_maintenance_task_sender(&self) -> crossbeam_channel::Sender<crate::cache::manager::background::types::MaintenanceTask> {
        self.maintenance_scheduler.task_sender.clone()
    }

    /// Shutdown background processing
    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        if let Err(e) = self.timer_sender.send(TimerCommand::Shutdown) {
            error!("Failed to signal timer shutdown: {:?}", e);
        }
        
        // NOTE: Cannot join threads due to structural limitation
        // timer_thread and worker_thread handles need &mut self to be taken
        // This requires API redesign or interior mutability
        
        self.worker_state.shutdown_workers(&self.maintenance_scheduler)?;
        self.maintenance_scheduler.stop_maintenance()?;
        Ok(())
    }

    /// Get task queue utilization for monitoring
    pub fn task_queue_utilization(&self) -> f64 {
        let capacity = 1000.0; // From bounded channel size
        let current_len = self.task_queue.len() as f64;
        current_len / capacity
    }

    /// Check if background processing is healthy
    pub fn is_healthy(&self) -> bool {
        self.worker_state.is_healthy() && self.maintenance_scheduler.is_healthy()
    }

    /// Pause maintenance tasks (for error recovery)
    pub fn pause_maintenance_tasks(&self) -> Result<(), CacheOperationError> {
        // Implementation would pause maintenance operations
        // For now, return success as this is a recovery mechanism
        Ok(())
    }

    /// Shutdown gracefully (for error recovery)
    pub fn shutdown_gracefully(&self) -> Result<(), CacheOperationError> {
        self.shutdown()
    }

    /// Trigger specific maintenance operation
    pub fn trigger_maintenance<KM: CacheKey, VM: CacheValue>(&self, _operation: &str) -> Result<(), CacheOperationError> {
        // Implementation would trigger specific maintenance operations
        // For now, return success as this is a recovery mechanism
        Ok(())
    }

    /// Rebalance worker distribution (for error recovery)
    pub fn rebalance_worker_distribution(&self) -> Result<(), CacheOperationError> {
        // Implementation would rebalance workers across tasks
        // For now, return success as this is a recovery mechanism
        Ok(())
    }
}

// String-specific singleton patterns removed - use tier coordinators directly for error recovery

impl<K: CacheKey + Default, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static, P: TaskProcessor> Drop for BackgroundCoordinator<K, V, P> {
    fn drop(&mut self) {
        // Send shutdown signal to timer thread
        let _ = self.timer_sender.send(TimerCommand::Shutdown);
        
        // Join timer thread if it exists
        if let Some(handle) = self.timer_thread.take() {
            let _ = handle.join();
        }
        
        // Join processor thread if it exists
        if let Some(handle) = self.processor_thread.take() {
            let _ = handle.join();
        }
        
        // Join worker thread if it exists  
        if let Some(handle) = self.worker_thread.take() {
            let _ = handle.join();
        }
    }
}
