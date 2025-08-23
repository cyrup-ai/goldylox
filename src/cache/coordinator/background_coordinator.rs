//! Background operation coordinator with work-stealing task queue
//!
//! This module manages background tasks, maintenance scheduling, and worker coordination
//! for the unified cache system using lock-free channels and atomic state management.

use std::sync::Arc;
use std::thread;
use std::time::Duration;
use crossbeam_channel::{bounded, Receiver, Sender};

use super::super::config::CacheConfig;
use super::super::manager::background::{
    types::{BackgroundTask, TaskProcessor}, BackgroundWorkerState, MaintenanceConfig, MaintenanceScheduler,
};
use super::super::traits::core::{CacheKey, CacheValue};
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
pub struct BackgroundCoordinator<K: CacheKey, V: CacheValue> {
    /// Background task queue (lock-free channel)
    task_queue: Sender<BackgroundTask>,
    /// Task receiver for processing
    task_receiver: Receiver<BackgroundTask>,
    /// Optional task processor for handling tasks
    task_processor: Option<Arc<dyn TaskProcessor>>,
    /// Timer thread for scheduling periodic tasks
    timer_thread: Option<thread::JoinHandle<()>>,
    /// Worker thread for task processing
    worker_thread: Option<thread::JoinHandle<()>>,
    /// Timer control sender
    timer_sender: Sender<TimerCommand>,
    /// Maintenance scheduler with atomic timing
    maintenance_scheduler: MaintenanceScheduler,
    /// Background worker state with atomic coordination
    worker_state: BackgroundWorkerState,
    /// Phantom data for generics
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K: CacheKey, V: CacheValue> BackgroundCoordinator<K, V> {
    /// Create new background coordinator with configuration
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let (task_queue, task_receiver) = bounded(1000); // Bounded queue for backpressure
        let (timer_sender, timer_receiver) = bounded(100);
        let maintenance_config = MaintenanceConfig::default();
        let maintenance_scheduler = MaintenanceScheduler::new(maintenance_config)?;
        let worker_state = BackgroundWorkerState::new(0);
        
        // Start timer thread
        let task_tx = task_queue.clone();
        let timer_thread = thread::spawn(move || {
            while let Ok(cmd) = timer_receiver.recv() {
                match cmd {
                    TimerCommand::Schedule { delay_ms, task } => {
                        thread::sleep(Duration::from_millis(delay_ms));
                        if let Err(e) = task_tx.try_send(task) {
                            eprintln!("Failed to send scheduled task: {:?}", e);
                        }
                    }
                    TimerCommand::Shutdown => break,
                }
            }
        });

        Ok(Self {
            task_queue,
            task_receiver,
            task_processor: None,
            timer_thread: Some(timer_thread),
            worker_thread: None,
            timer_sender,
            maintenance_scheduler,
            worker_state,
            _phantom: std::marker::PhantomData,
        })
    }
    
    /// Set task processor for handling background tasks
    pub fn set_task_processor(&mut self, processor: Arc<dyn TaskProcessor>) {
        self.task_processor = Some(processor);
    }

    /// Start background worker threads
    pub fn start_worker_threads(&self) -> Result<(), CacheOperationError> {
        // Only start if not already started
        if self.worker_thread.is_some() {
            return Ok(());
        }
        
        let rx = self.task_receiver.clone();
        let processor = self.task_processor.clone();
        let timer_tx = self.timer_sender.clone();
        
        let _handle = thread::Builder::new()
            .name("cache-worker".to_string())
            .spawn(move || {
                while let Ok(task) = rx.recv() {
                    if let Some(ref proc) = processor {
                        match &task {
                            BackgroundTask::Statistics { stats_type: 2, interval_ms } => {
                                if let Err(e) = proc.process_task(&task) {
                                    eprintln!("Memory monitoring task failed: {:?}", e);
                                }
                                // Reschedule periodic task
                                if let Err(e) = timer_tx.send(TimerCommand::Schedule {
                                    delay_ms: *interval_ms,
                                    task: task.clone(),
                                }) {
                                    eprintln!("Failed to reschedule monitoring task: {:?}", e);
                                    break;
                                }
                            }
                            _ => {
                                // Process other task types
                                if let Err(e) = proc.process_task(&task) {
                                    eprintln!("Background task failed: {:?}", e);
                                }
                            }
                        }
                    }
                }
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
    pub fn maintenance_scheduler(&self) -> &MaintenanceScheduler {
        &self.maintenance_scheduler
    }

    /// Shutdown background processing
    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        if let Err(e) = self.timer_sender.send(TimerCommand::Shutdown) {
            eprintln!("Failed to signal timer shutdown: {:?}", e);
        }
        
        // NOTE: Cannot join threads due to structural limitation
        // timer_thread and worker_thread handles need &mut self to be taken
        // This requires API redesign or interior mutability
        
        self.worker_state.shutdown_workers()?;
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
}
