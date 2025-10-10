#![allow(dead_code)]
// Worker System - Complete task coordination library with command queues, synchronous task execution using crossbeam messaging, execution contexts, deferred mutations, statistics tracking, and safe coordination between tasks and cache state

//! Task coordination and command queue system for cache operations
//!
//! This module provides safe coordination between tasks and cache state using pure crossbeam
//! messaging patterns, implementing command queue patterns for deferred mutations with
//! lock-free atomic operations and dedicated worker threads.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
// Arc removed - using crossbeam messaging instead
use crossbeam_channel::{Receiver, Sender, bounded};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use log::warn;

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::CacheTier;
// Task coordination is handled directly through MaintenanceScheduler

/// Type alias for complex task operation function
type TaskOperation<K, V> =
    Box<dyn FnOnce(TaskExecutionContext<K, V>) -> Result<(), CacheOperationError> + Send + 'static>;

/// Command queue for safe cache mutations using crossbeam channels
#[derive(Debug)]
pub struct CacheCommandQueue<K: CacheKey, V: CacheValue> {
    /// Channel sender for commands
    sender: Sender<CacheCommand<K, V>>,
    /// Channel receiver for commands (wrapped in Option for taking)
    receiver: Option<Receiver<CacheCommand<K, V>>>,
    /// Command execution statistics
    stats: CommandQueueStats,
    /// Maximum queue size
    max_queue_size: usize,
}

/// Individual cache command for deferred execution
#[derive(Debug, Clone)]
pub enum CacheCommand<K: CacheKey, V: CacheValue> {
    /// Insert entry into cache tier
    Insert {
        key: K,
        value: V,
        tier: CacheTier,
        timestamp: Instant,
    },
    /// Remove entry from cache tier
    Remove {
        key: K,
        tier: CacheTier,
        timestamp: Instant,
    },
    /// Prefetch key into cache (internal optimization)
    Prefetch {
        key: K,
        confidence: f64,
        timestamp: Instant,
    },
    /// Move entry between tiers
    Move {
        key: K,
        from_tier: CacheTier,
        to_tier: CacheTier,
        timestamp: Instant,
    },
    /// Update entry metadata
    UpdateMetadata {
        key: K,
        tier: CacheTier,
        access_count: u64,
        last_access: Instant,
        timestamp: Instant,
    },
    /// Flush dirty entries to storage
    FlushDirty {
        keys: Vec<K>,
        tier: CacheTier,
        timestamp: Instant,
    },
    /// Compact cache tier
    Compact {
        tier: CacheTier,
        target_size: usize,
        timestamp: Instant,
    },
}

/// Command queue statistics
#[derive(Debug)]
pub struct CommandQueueStats {
    /// Total commands processed
    total_commands: AtomicU64,
    /// Commands currently queued
    queued_commands: AtomicUsize,
    /// Maximum queue depth reached
    max_queue_depth: AtomicUsize,
    /// Average command execution time
    avg_execution_time: AtomicU64, // Nanoseconds
    /// Command processing throughput
    throughput: AtomicU64, // Commands per second
}

impl Clone for CommandQueueStats {
    fn clone(&self) -> Self {
        Self {
            total_commands: AtomicU64::new(
                self.total_commands
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            queued_commands: AtomicUsize::new(
                self.queued_commands
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            max_queue_depth: AtomicUsize::new(
                self.max_queue_depth
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            avg_execution_time: AtomicU64::new(
                self.avg_execution_time
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            throughput: AtomicU64::new(self.throughput.load(std::sync::atomic::Ordering::Relaxed)),
        }
    }
}

/// Configuration for TaskCoordinator
#[derive(Debug, Clone)]
pub struct TaskCoordinatorConfig {
    /// Maximum number of keys allowed per task
    pub max_keys_per_task: usize,
    /// Default timeout for task operations
    pub default_task_timeout: Duration,
}

impl Default for TaskCoordinatorConfig {
    fn default() -> Self {
        Self {
            max_keys_per_task: 10000,
            default_task_timeout: Duration::from_secs(30),
        }
    }
}

/// Task coordinator for managing cache operations
#[derive(Debug)]
pub struct TaskCoordinator<
    K: CacheKey + Default,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> {
    /// Command queue for safe mutations
    command_queue: CacheCommandQueue<K, V>,
    /// Active task tracking
    active_tasks: DashMap<u64, TaskInfo<K>>,
    /// Task execution worker communication channel
    task_worker_sender: crossbeam_channel::Sender<TaskCommand<K, V>>,
    /// Task ID generator
    next_task_id: AtomicU64,
    /// Coordination statistics
    stats: CoordinatorStats,
    /// Shutdown flag
    shutdown: AtomicBool,
    /// Worker thread handle for graceful shutdown
    worker_thread: Option<std::thread::JoinHandle<()>>,
    /// Configuration settings
    config: TaskCoordinatorConfig,
}

/// Commands for the task execution worker
pub enum TaskCommand<
    K: CacheKey + Default,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> {
    Execute {
        task_id: u64,
        operation: TaskOperation<K, V>,
        context: TaskExecutionContext<K, V>,
    },
    Cancel {
        task_id: u64,
        response: crossbeam_channel::Sender<bool>,
    },
    GetActiveCount {
        response: crossbeam_channel::Sender<usize>,
    },
    Shutdown,
}

/// Worker that executes tasks synchronously using crossbeam messaging
pub struct TaskExecutionWorker<
    K: CacheKey + Default,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> {
    receiver: crossbeam_channel::Receiver<TaskCommand<K, V>>,
    active_tasks: std::collections::HashMap<u64, TaskExecutionState>,
    stats: TaskWorkerStats,
    startup_time: Instant,
    active_tasks_tracker: DashMap<u64, TaskInfo<K>>,
    coordinator_stats: CoordinatorStats,
}

#[derive(Debug)]
struct TaskExecutionState {
    start_time: Instant,
    task_type: String,
}

#[derive(Debug)]
pub struct TaskWorkerStats {
    pub total_commands_processed: AtomicU64,
    pub total_executed: AtomicU64,
    pub total_successful: AtomicU64,
    pub total_cancelled: AtomicU64,
    pub total_failed: AtomicU64,
    pub worker_start_time: Instant,
    /// Error accumulator for tracking failure patterns
    pub recent_errors: std::sync::RwLock<std::collections::VecDeque<(Instant, String)>>,
    /// Maximum number of recent errors to track
    pub max_error_history: usize,
}

impl TaskWorkerStats {
    pub fn new() -> Self {
        Self {
            total_commands_processed: AtomicU64::new(0),
            total_executed: AtomicU64::new(0),
            total_successful: AtomicU64::new(0),
            total_cancelled: AtomicU64::new(0),
            total_failed: AtomicU64::new(0),
            worker_start_time: Instant::now(),
            recent_errors: std::sync::RwLock::new(std::collections::VecDeque::new()),
            max_error_history: 100,
        }
    }

    /// Calculate success rate as percentage (0.0-100.0)
    pub fn success_rate_percent(&self) -> f64 {
        let executed = self
            .total_executed
            .load(std::sync::atomic::Ordering::Relaxed);
        let successful = self
            .total_successful
            .load(std::sync::atomic::Ordering::Relaxed);

        if executed == 0 {
            0.0
        } else {
            (successful as f64 / executed as f64) * 100.0
        }
    }

    /// Record a failed operation with error details
    pub fn record_error(&self, error_msg: String) -> Result<(), CacheOperationError> {
        self.total_failed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        match self.recent_errors.write() {
            Ok(mut errors) => {
                // Add new error
                errors.push_back((Instant::now(), error_msg));

                // Maintain maximum error history size
                while errors.len() > self.max_error_history {
                    errors.pop_front();
                }
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to acquire write lock for error recording: {:?}", e);
                Err(CacheOperationError::InvalidState(
                    "Error tracking lock poisoned".to_string(),
                ))
            }
        }
    }

    /// Get recent error patterns for debugging
    pub fn get_recent_errors(
        &self,
        max_age: std::time::Duration,
    ) -> Result<Vec<(Instant, String)>, CacheOperationError> {
        match self.recent_errors.read() {
            Ok(errors) => {
                let now = Instant::now();
                let filtered = errors
                    .iter()
                    .filter(|(timestamp, _)| now.duration_since(*timestamp) <= max_age)
                    .cloned()
                    .collect();
                Ok(filtered)
            }
            Err(e) => {
                log::error!("Failed to acquire read lock for error retrieval: {:?}", e);
                Err(CacheOperationError::InvalidState(
                    "Error tracking lock poisoned".to_string(),
                ))
            }
        }
    }

    /// Get failure rate as percentage (0.0-100.0)
    pub fn failure_rate_percent(&self) -> f64 {
        let executed = self
            .total_executed
            .load(std::sync::atomic::Ordering::Relaxed);
        let failed = self.total_failed.load(std::sync::atomic::Ordering::Relaxed);

        if executed == 0 {
            0.0
        } else {
            (failed as f64 / executed as f64) * 100.0
        }
    }
}

impl<
    K: CacheKey + Default,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> TaskExecutionWorker<K, V>
{
    pub fn new(
        receiver: crossbeam_channel::Receiver<TaskCommand<K, V>>,
        active_tasks_tracker: DashMap<u64, TaskInfo<K>>,
        coordinator_stats: CoordinatorStats,
    ) -> Self {
        Self {
            receiver,
            active_tasks: std::collections::HashMap::new(),
            stats: TaskWorkerStats::new(),
            startup_time: Instant::now(),
            active_tasks_tracker,
            coordinator_stats,
        }
    }

    pub fn run(mut self) {
        log::info!("TaskExecutionWorker starting up at {:?}", self.startup_time);

        while let Ok(command) = self.receiver.recv() {
            self.stats
                .total_commands_processed
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            match command {
                TaskCommand::Execute {
                    task_id,
                    operation,
                    context,
                } => {
                    log::debug!("Executing task_id: {}", task_id);

                    let execution_state = TaskExecutionState {
                        start_time: context.start_time,
                        task_type: context.metadata.task_type.clone(),
                    };

                    self.active_tasks.insert(task_id, execution_state);

                    let result = operation(context);
                    let task_duration =
                        if let Some(execution_state) = self.active_tasks.get(&task_id) {
                            execution_state.start_time.elapsed()
                        } else {
                            log::warn!(
                                "Task {} not found in active_tasks during completion",
                                task_id
                            );
                            std::time::Duration::from_secs(0)
                        };

                    self.coordinator_stats
                        .update_task_completion(task_duration, result.is_ok());
                    self.active_tasks_tracker.remove(&task_id);
                    self.coordinator_stats
                        .active_task_count
                        .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

                    self.active_tasks.remove(&task_id);
                    self.stats
                        .total_executed
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    match &result {
                        Ok(_) => {
                            self.stats
                                .total_successful
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            log::debug!("Completed task_id: {} successfully", task_id);
                        }
                        Err(error) => {
                            if let Err(record_err) = self
                                .stats
                                .record_error(format!("Task {}: {}", task_id, error))
                            {
                                log::error!(
                                    "Failed to record error for task {}: {:?}",
                                    task_id,
                                    record_err
                                );
                            }
                            log::warn!("Task {} failed with error: {}", task_id, error);
                        }
                    }
                }

                TaskCommand::Cancel { task_id, response } => {
                    let cancelled = self.active_tasks.remove(&task_id).is_some();
                    if cancelled {
                        self.active_tasks_tracker.remove(&task_id);
                        self.coordinator_stats
                            .active_task_count
                            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        self.stats
                            .total_cancelled
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        log::debug!("Cancelled task_id: {}", task_id);
                    } else {
                        log::warn!("Attempted to cancel non-existent task_id: {}", task_id);
                    }

                    if let Err(e) = response.send(cancelled) {
                        log::error!(
                            "Failed to send cancel response for task_id: {}. Error: {:?}",
                            task_id,
                            e
                        );
                    }
                }

                TaskCommand::GetActiveCount { response } => {
                    let count = self.active_tasks.len();
                    if let Err(e) = response.send(count) {
                        log::error!("Failed to send active count response. Error: {:?}", e);
                    }
                }

                TaskCommand::Shutdown => {
                    log::info!("TaskExecutionWorker received shutdown command");
                    break;
                }
            }
        }

        let remaining_count = self.active_tasks.len();
        if remaining_count > 0 {
            log::info!(
                "TaskExecutionWorker shutting down with {} remaining tasks",
                remaining_count
            );
            for task_id in self.active_tasks.keys() {
                self.active_tasks_tracker.remove(task_id);
            }
        }

        let total_runtime = self.startup_time.elapsed();
        log::info!(
            "TaskExecutionWorker shutdown complete. Runtime: {:?}, Stats: executed={}, successful={}, failed={}, cancelled={}",
            total_runtime,
            self.stats
                .total_executed
                .load(std::sync::atomic::Ordering::Relaxed),
            self.stats
                .total_successful
                .load(std::sync::atomic::Ordering::Relaxed),
            self.stats
                .total_failed
                .load(std::sync::atomic::Ordering::Relaxed),
            self.stats
                .total_cancelled
                .load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}

/// Task execution status
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    /// Task is waiting to be executed
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with an error
    Failed,
    /// Task was cancelled
    Cancelled,
}

impl TaskStatus {
    /// Convert status to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

/// Information about active tasks
#[derive(Debug, Clone)]
pub struct TaskInfo<K: CacheKey> {
    /// Task ID
    id: u64,
    /// Task type description
    task_type: String,
    /// Task priority level
    priority: u16,
    /// Start time
    started_at: Instant,
    /// Estimated completion time
    estimated_completion: Option<Instant>,
    /// Current task status
    status: TaskStatus,
    /// Task progress (0.0 to 1.0)
    progress: f32,
    /// Associated cache keys
    keys: Vec<K>, // Generic keys for tracking
}

impl<K: CacheKey> TaskInfo<K> {
    /// Get the task ID
    pub fn task_id(&self) -> u64 {
        self.id
    }

    /// Get the task type
    pub fn task_type(&self) -> &str {
        &self.task_type
    }

    /// Get current task status
    pub fn status(&self) -> &TaskStatus {
        &self.status
    }

    /// Get task progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Update task status
    pub fn update_status(&mut self, status: TaskStatus) {
        self.status = status;
    }

    /// Update task progress
    pub fn update_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    /// Get task priority
    pub fn priority(&self) -> u16 {
        self.priority
    }

    /// Get task start time as elapsed duration since creation
    pub fn elapsed_time(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Get approximate start timestamp (current time - elapsed time)
    pub fn start_timestamp(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let elapsed_secs = self.started_at.elapsed().as_secs();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(elapsed_secs)
    }
}

/// Coordinator statistics
#[derive(Debug)]
pub struct CoordinatorStats {
    /// Total tasks coordinated
    total_tasks: AtomicU64,
    /// Currently active tasks
    active_task_count: AtomicUsize,
    /// Task completion rate
    completion_rate: AtomicU64, // Tasks per second
    /// Average task duration
    avg_task_duration: AtomicU64, // Nanoseconds
    /// Task success count
    successful_tasks: AtomicU64,
    /// Task completion count
    completed_tasks: AtomicU64,
}

/// Task execution context for cache operations
#[derive(Debug)]
pub struct TaskExecutionContext<K: CacheKey, V: CacheValue, T = ()> {
    /// Command queue for mutations
    command_sender: Sender<CacheCommand<K, V>>,
    /// Task ID for tracking
    task_id: u64,
    /// Execution start time
    start_time: Instant,
    /// Task metadata
    metadata: TaskMetadata,
    /// Result coordination channel (NEW for task result integration)
    pub result_sender: Option<crossbeam_channel::Sender<Result<T, CacheOperationError>>>,
}

/// Task metadata for execution tracking
#[derive(Debug, Clone)]
pub struct TaskMetadata {
    /// Task type description
    task_type: String,
    /// Priority level
    priority: u16,
    /// Expected duration
    expected_duration: Duration,
    /// Retry count
    retry_count: u32,
    /// NUMA node preference
    numa_node: Option<usize>,
}

impl<K: CacheKey, V: CacheValue> CacheCommandQueue<K, V> {
    /// Create new command queue
    pub fn new(max_queue_size: usize) -> Self {
        let (sender, receiver) = bounded(max_queue_size);
        Self {
            sender,
            receiver: Some(receiver),
            stats: CommandQueueStats::new(),
            max_queue_size,
        }
    }

    /// Enqueue command for deferred execution
    pub fn enqueue_command(&self, command: CacheCommand<K, V>) -> Result<(), CacheOperationError> {
        // Try to send command through channel
        self.sender.try_send(command).map_err(|_| {
            CacheOperationError::ResourceExhausted("Command queue full".to_string())
        })?;

        self.stats.queued_commands.fetch_add(1, Ordering::Relaxed);

        // Update max queue depth (approximate, since channel doesn't expose len())
        let current_depth = self.stats.queued_commands.load(Ordering::Relaxed);
        self.stats
            .max_queue_depth
            .fetch_max(current_depth, Ordering::Relaxed);

        Ok(())
    }

    /// Drain and execute all pending commands
    pub fn execute_pending_commands<F>(
        &mut self,
        mut executor: F,
    ) -> Result<usize, CacheOperationError>
    where
        F: FnMut(CacheCommand<K, V>) -> Result<(), CacheOperationError>,
    {
        // Take the receiver out (can only be done once)
        let receiver = self.receiver.take().ok_or_else(|| {
            CacheOperationError::InvalidState("Receiver already taken".to_string())
        })?;

        let execution_start = Instant::now();
        let mut command_count = 0;

        // Use try_iter() to drain all pending commands without blocking
        for command in receiver.try_iter() {
            let command_start = Instant::now();
            command_count += 1;

            // Execute command
            if let Err(e) = executor(command) {
                // Log error but continue processing other commands
                warn!("Command execution failed: {:?}", e);
            }

            // Update statistics
            let execution_time = command_start.elapsed();
            self.update_execution_stats(execution_time);
        }

        // Put the receiver back
        self.receiver = Some(receiver);

        // Update queue statistics
        self.stats.queued_commands.store(0, Ordering::Relaxed);
        self.stats
            .total_commands
            .fetch_add(command_count as u64, Ordering::Relaxed);

        // Update throughput
        let total_time = execution_start.elapsed();
        if total_time.as_secs() > 0 {
            let throughput = command_count as u64 / total_time.as_secs();
            self.stats.throughput.store(throughput, Ordering::Relaxed);
        }

        Ok(command_count)
    }

    /// Drain all pending commands without executing them
    /// 
    /// Extracts all commands from the internal queue, leaving it empty.
    /// This enables async processing without blocking callbacks.
    /// 
    /// Returns a Vec containing all pending commands.
    pub fn drain_pending_commands(&mut self) -> Result<Vec<CacheCommand<K, V>>, CacheOperationError> {
        // Take the receiver out (same pattern as execute_pending_commands)
        let receiver = self.receiver.take().ok_or_else(|| {
            CacheOperationError::InvalidState("Receiver already taken".to_string())
        })?;

        // Drain all pending commands into a Vec using try_iter() and collect()
        let commands: Vec<CacheCommand<K, V>> = receiver.try_iter().collect();
        let command_count = commands.len();

        // Put the receiver back (critical - must always restore)
        self.receiver = Some(receiver);

        // Update queue statistics (same as execute_pending_commands)
        self.stats.queued_commands.store(0, Ordering::Relaxed);
        self.stats
            .total_commands
            .fetch_add(command_count as u64, Ordering::Relaxed);

        Ok(commands)
    }

    /// Update execution statistics
    fn update_execution_stats(&self, execution_time: Duration) {
        let execution_ns = execution_time.as_nanos() as u64;

        // Update average execution time using exponential moving average
        let current_avg = self.stats.avg_execution_time.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            execution_ns
        } else {
            // EMA with alpha = 0.1
            (current_avg * 9 + execution_ns) / 10
        };
        self.stats
            .avg_execution_time
            .store(new_avg, Ordering::Relaxed);
    }

    /// Get command queue statistics
    pub fn get_stats(&self) -> CommandQueueStatsSnapshot {
        CommandQueueStatsSnapshot {
            total_commands: self.stats.total_commands.load(Ordering::Relaxed),
            queued_commands: self.stats.queued_commands.load(Ordering::Relaxed),
            max_queue_depth: self.stats.max_queue_depth.load(Ordering::Relaxed),
            avg_execution_time_ns: self.stats.avg_execution_time.load(Ordering::Relaxed),
            throughput: self.stats.throughput.load(Ordering::Relaxed),
        }
    }

    /// Clear all pending commands
    pub fn clear(&mut self) -> Result<usize, CacheOperationError> {
        // Take the receiver and drain it
        let receiver = self.receiver.take().ok_or_else(|| {
            CacheOperationError::InvalidState("Receiver already taken".to_string())
        })?;

        let cleared_count = receiver.try_iter().count();
        self.receiver = Some(receiver);

        self.stats.queued_commands.store(0, Ordering::Relaxed);

        Ok(cleared_count)
    }

    /// Get a sender that can be cloned for use in other contexts
    pub fn get_sender(&self) -> Sender<CacheCommand<K, V>> {
        self.sender.clone()
    }
}

// Manual Clone implementation since Receiver can't be cloned
impl<K: CacheKey, V: CacheValue> Clone for CacheCommandQueue<K, V> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            receiver: None, // Only the original keeps the receiver
            stats: self.stats.clone(),
            max_queue_size: self.max_queue_size,
        }
    }
}

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()>,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> TaskCoordinator<K, V>
{
    /// Create new task coordinator with pure crossbeam task execution
    pub fn new_direct(max_command_queue_size: usize) -> Result<Self, CacheOperationError> {
        Self::new_with_config(max_command_queue_size, TaskCoordinatorConfig::default())
    }

    /// Create new task coordinator with custom configuration
    pub fn new_with_config(
        max_command_queue_size: usize,
        config: TaskCoordinatorConfig,
    ) -> Result<Self, CacheOperationError> {
        let command_queue = CacheCommandQueue::new(max_command_queue_size);
        let active_tasks = DashMap::new();
        let stats = CoordinatorStats::new();

        let (task_worker_sender, task_worker_receiver) = bounded(1000);
        let task_worker =
            TaskExecutionWorker::new(task_worker_receiver, active_tasks.clone(), stats.clone());

        let worker_handle = std::thread::Builder::new()
            .name("task-execution-worker".to_string())
            .spawn(move || {
                task_worker.run();
            })
            .map_err(|e| {
                log::error!("Failed to spawn TaskExecutionWorker thread: {}", e);
                CacheOperationError::initialization_failed(format!(
                    "TaskExecutionWorker thread spawn failed: {}",
                    e
                ))
            })?;

        log::info!("TaskCoordinator initialized with dedicated TaskExecutionWorker thread");

        Ok(Self {
            command_queue,
            active_tasks,
            task_worker_sender,
            next_task_id: AtomicU64::new(1),
            stats,
            shutdown: AtomicBool::new(false),
            worker_thread: Some(worker_handle),
            config,
        })
    }

    /// Schedule cache operation task
    pub fn schedule_cache_operation<F, T>(
        &self,
        operation: F,
        task_type: String,
        priority: u16,
        keys: Vec<K>,
    ) -> Result<u64, CacheOperationError>
    where
        F: FnOnce(TaskExecutionContext<K, V>) -> Result<T, CacheOperationError> + Send + 'static,
        T: Send + 'static,
    {
        let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
        let start_time = Instant::now();

        let task_info = TaskInfo::<K> {
            id: task_id,
            task_type: task_type.clone(),
            priority,
            started_at: start_time,
            estimated_completion: None,
            status: TaskStatus::Running,
            progress: 0.0,
            keys,
        };

        self.active_tasks.insert(task_id, task_info);
        self.stats.active_task_count.fetch_add(1, Ordering::Relaxed);

        let context = TaskExecutionContext {
            command_sender: self.command_queue.get_sender(),
            task_id,
            start_time,
            metadata: TaskMetadata {
                task_type,
                priority,
                expected_duration: Duration::from_millis(100),
                retry_count: 0,
                numa_node: None,
            },
            result_sender: None,
        };

        let wrapped_operation = Box::new(
            move |ctx: TaskExecutionContext<K, V>| -> Result<(), CacheOperationError> {
                operation(ctx).map(|_| ())
            },
        );

        let task_command = TaskCommand::Execute {
            task_id,
            operation: wrapped_operation,
            context,
        };

        if self.task_worker_sender.try_send(task_command).is_err() {
            self.active_tasks.remove(&task_id);
            self.stats.active_task_count.fetch_sub(1, Ordering::Relaxed);
            return Err(CacheOperationError::resource_exhausted(
                "Task worker queue full",
            ));
        }

        self.stats.total_tasks.fetch_add(1, Ordering::Relaxed);
        Ok(task_id)
    }

    /// Schedule cache operation task with result coordination
    pub fn schedule_cache_operation_with_result<F, T>(
        &self,
        operation: F,
        task_type: String,
        priority: u16,
        keys: Vec<K>,
        result_sender: Option<crossbeam_channel::Sender<Result<T, CacheOperationError>>>,
    ) -> Result<u64, CacheOperationError>
    where
        F: FnOnce(TaskExecutionContext<K, V, T>) -> Result<T, CacheOperationError> + Send + 'static,
        T: Send + 'static,
    {
        let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
        let start_time = Instant::now();

        let task_info = TaskInfo::<K> {
            id: task_id,
            task_type: task_type.clone(),
            priority,
            started_at: start_time,
            estimated_completion: None,
            status: TaskStatus::Running,
            progress: 0.0,
            keys,
        };

        self.active_tasks.insert(task_id, task_info);
        self.stats.active_task_count.fetch_add(1, Ordering::Relaxed);

        let wrapped_operation = Box::new(
            move |ctx: TaskExecutionContext<K, V>| -> Result<(), CacheOperationError> {
                let result = operation(TaskExecutionContext {
                    command_sender: ctx.command_sender,
                    task_id: ctx.task_id,
                    start_time: ctx.start_time,
                    metadata: ctx.metadata,
                    result_sender: result_sender.clone(),
                });

                let operation_success = result.is_ok();

                if let Some(sender) = result_sender {
                    if let Err(send_error) = sender.send(result) {
                        log::error!(
                            "Failed to send task result for task {}: {:?}",
                            ctx.task_id,
                            send_error
                        );
                        return Err(CacheOperationError::InvalidState(format!(
                            "Failed to deliver task result: {:?}",
                            send_error
                        )));
                    } else {
                        log::debug!("Successfully sent result for task {}", ctx.task_id);
                    }
                }

                // Return success/failure based on original operation result
                if operation_success {
                    Ok(())
                } else {
                    Err(CacheOperationError::OperationFailed)
                }
            },
        );

        let context = TaskExecutionContext {
            command_sender: self.command_queue.get_sender(),
            task_id,
            start_time,
            metadata: TaskMetadata {
                task_type,
                priority,
                expected_duration: Duration::from_millis(100),
                retry_count: 0,
                numa_node: None,
            },
            result_sender: None,
        };

        let task_command = TaskCommand::Execute {
            task_id,
            operation: wrapped_operation,
            context,
        };

        if self.task_worker_sender.try_send(task_command).is_err() {
            self.active_tasks.remove(&task_id);
            self.stats.active_task_count.fetch_sub(1, Ordering::Relaxed);
            return Err(CacheOperationError::resource_exhausted(
                "Task worker queue full",
            ));
        }

        self.stats.total_tasks.fetch_add(1, Ordering::Relaxed);
        Ok(task_id)
    }

    /// Schedule cache operation with predetermined task ID for coordination
    pub fn schedule_cache_operation_with_task_id<F, T>(
        &self,
        operation: F,
        task_type: String,
        priority: u16,
        keys: Vec<K>,
        result_sender: Option<crossbeam_channel::Sender<Result<T, CacheOperationError>>>,
        predetermined_task_id: u64,
    ) -> Result<u64, CacheOperationError>
    where
        F: FnOnce(TaskExecutionContext<K, V, T>) -> Result<T, CacheOperationError> + Send + 'static,
        T: Send + 'static,
    {
        if predetermined_task_id == 0 {
            return Err(CacheOperationError::InvalidArgument(
                "Task ID cannot be zero".to_string(),
            ));
        }
        if keys.len() > self.config.max_keys_per_task {
            return Err(CacheOperationError::InvalidArgument(format!(
                "Too many keys (max {})",
                self.config.max_keys_per_task
            )));
        }
        if task_type.is_empty() {
            return Err(CacheOperationError::InvalidArgument(
                "Task type cannot be empty".to_string(),
            ));
        }

        let task_id = predetermined_task_id;
        let start_time = Instant::now();

        let task_info = TaskInfo::<K> {
            id: task_id,
            task_type: task_type.clone(),
            priority,
            started_at: start_time,
            estimated_completion: None,
            status: TaskStatus::Running,
            progress: 0.0,
            keys,
        };

        self.active_tasks.insert(task_id, task_info);
        self.stats.active_task_count.fetch_add(1, Ordering::Relaxed);

        let wrapped_operation = Box::new(
            move |ctx: TaskExecutionContext<K, V>| -> Result<(), CacheOperationError> {
                let result = operation(TaskExecutionContext {
                    command_sender: ctx.command_sender,
                    task_id: ctx.task_id,
                    start_time: ctx.start_time,
                    metadata: ctx.metadata,
                    result_sender: result_sender.clone(),
                });

                let operation_success = result.is_ok();

                if let Some(sender) = result_sender {
                    if let Err(send_error) = sender.send(result) {
                        log::error!(
                            "Failed to send task result for task {}: {:?}",
                            ctx.task_id,
                            send_error
                        );
                        return Err(CacheOperationError::InvalidState(format!(
                            "Failed to deliver task result: {:?}",
                            send_error
                        )));
                    } else {
                        log::debug!("Successfully sent result for task {}", ctx.task_id);
                    }
                }

                // Return success/failure based on original operation result
                if operation_success {
                    Ok(())
                } else {
                    Err(CacheOperationError::OperationFailed)
                }
            },
        );

        let context = TaskExecutionContext {
            command_sender: self.command_queue.get_sender(),
            task_id,
            start_time,
            metadata: TaskMetadata {
                task_type,
                priority,
                expected_duration: Duration::from_millis(100),
                retry_count: 0,
                numa_node: None,
            },
            result_sender: None,
        };

        let task_command = TaskCommand::Execute {
            task_id,
            operation: wrapped_operation,
            context,
        };

        if self.task_worker_sender.try_send(task_command).is_err() {
            self.active_tasks.remove(&task_id);
            self.stats.active_task_count.fetch_sub(1, Ordering::Relaxed);
            return Err(CacheOperationError::resource_exhausted(
                "Task worker queue full",
            ));
        }

        self.stats.total_tasks.fetch_add(1, Ordering::Relaxed);
        Ok(task_id)
    }

    /// Get next task ID without incrementing (for external coordination)
    pub fn next_task_id(&self) -> u64 {
        self.next_task_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Execute all pending commands
    pub fn flush_command_queue<F>(&mut self, executor: F) -> Result<usize, CacheOperationError>
    where
        F: FnMut(CacheCommand<K, V>) -> Result<(), CacheOperationError>,
    {
        self.command_queue.execute_pending_commands(executor)
    }

    /// Drain all pending commands for async processing
    /// 
    /// Extracts all commands from the internal queue, leaving it empty.
    /// This method enables async processing without blocking callbacks.
    /// 
    /// # Returns
    /// 
    /// A Vec containing all pending commands. The internal queue will be empty
    /// after this call.
    /// 
    /// # Example
    /// 
    /// ```rust,ignore
    /// let commands = task_coordinator.drain_pending_commands()?;
    /// for command in commands {
    ///     // Process asynchronously
    ///     process_command(command).await;
    /// }
    /// ```
    pub fn drain_pending_commands(&mut self) -> Result<Vec<CacheCommand<K, V>>, CacheOperationError> {
        self.command_queue.drain_pending_commands()
    }

    /// Enqueue command for background processing
    pub fn enqueue_command(&self, command: CacheCommand<K, V>) -> Result<(), CacheOperationError> {
        self.command_queue.enqueue_command(command)
    }

    /// Get coordinator statistics
    pub fn get_stats(&self) -> CoordinatorStatsSnapshot {
        CoordinatorStatsSnapshot {
            total_tasks: self.stats.total_tasks.load(Ordering::Relaxed),
            active_task_count: self.stats.active_task_count.load(Ordering::Relaxed),
            completion_rate: self.stats.completion_rate.load(Ordering::Relaxed),
            avg_task_duration_ns: self.stats.avg_task_duration.load(Ordering::Relaxed),
            success_rate_percent: self.stats.success_rate_percent(),
            command_queue_stats: self.command_queue.get_stats(),
        }
    }

    /// Get active task information
    pub fn get_active_tasks(&self) -> Vec<TaskInfo<K>> {
        self.active_tasks
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Cancel active task using crossbeam messaging to TaskHandleWorker
    pub fn cancel_task(&self, task_id: u64) -> Result<bool, CacheOperationError> {
        // Send cancel command to the TaskHandleWorker through crossbeam channel
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);

        let task_cancelled = match self.task_worker_sender.try_send(TaskCommand::Cancel {
            task_id,
            response: response_sender,
        }) {
            Ok(()) => {
                // Successfully sent command, wait for response with reasonable timeout
                match response_receiver.recv_timeout(std::time::Duration::from_secs(5)) {
                    Ok(cancelled) => {
                        log::debug!(
                            "TaskExecutionWorker responded: task_id {} cancel result = {}",
                            task_id,
                            cancelled
                        );
                        cancelled
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        log::error!(
                            "Timeout waiting for cancel response from TaskExecutionWorker for task_id: {}",
                            task_id
                        );
                        false
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        log::error!(
                            "TaskExecutionWorker channel disconnected while canceling task_id: {}",
                            task_id
                        );
                        false
                    }
                }
            }
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                log::error!(
                    "TaskExecutionWorker command queue is full, cannot cancel task_id: {}",
                    task_id
                );
                return Err(CacheOperationError::ResourceExhausted(
                    "Task execution worker queue is full".to_string(),
                ));
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                log::error!(
                    "TaskExecutionWorker channel disconnected, cannot cancel task_id: {}",
                    task_id
                );
                return Err(CacheOperationError::InvalidState(
                    "Task execution worker is not available".to_string(),
                ));
            }
        };

        // Only remove from active tasks tracking if worker confirmed cancellation
        if task_cancelled {
            let was_active = self.active_tasks.remove(&task_id).is_some();
            if was_active {
                self.stats.active_task_count.fetch_sub(1, Ordering::Relaxed);
            }
            log::debug!(
                "Task cancellation completed: task_id={}, was_active={}, worker_confirmed={}",
                task_id,
                was_active,
                task_cancelled
            );
        } else {
            log::debug!(
                "Task cancellation failed: task_id={} not cancelled by worker",
                task_id
            );
        }

        Ok(task_cancelled)
    }

    /// Shutdown coordinator gracefully
    pub fn shutdown(&mut self) -> Result<(), CacheOperationError> {
        self.shutdown.store(true, Ordering::Relaxed);

        // Clear command queue
        let _cleared = self.command_queue.clear()?;

        // Cancel all active tasks
        let active_task_ids: Vec<_> = self.active_tasks.iter().map(|entry| *entry.key()).collect();
        for task_id in active_task_ids {
            let _ = self.cancel_task(task_id);
        }

        // Send shutdown command to worker
        if let Err(e) = self.task_worker_sender.try_send(TaskCommand::Shutdown) {
            log::warn!("Failed to send shutdown command to worker: {:?}", e);
        }

        // Wait for worker thread to terminate
        if let Some(handle) = self.worker_thread.take() {
            match handle.join() {
                Ok(_) => {
                    log::info!("TaskExecutionWorker thread terminated gracefully");
                }
                Err(e) => {
                    log::error!(
                        "TaskExecutionWorker thread panicked during shutdown: {:?}",
                        e
                    );
                    return Err(CacheOperationError::InvalidState(
                        "Worker thread panicked".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl<K: CacheKey, V: CacheValue, T> TaskExecutionContext<K, V, T> {
    /// Enqueue command for deferred execution
    pub fn enqueue_command(&self, command: CacheCommand<K, V>) -> Result<(), CacheOperationError> {
        self.command_sender
            .try_send(command)
            .map_err(|_| CacheOperationError::ResourceExhausted("Command queue full".to_string()))
    }

    /// Get task execution time so far
    pub fn elapsed_time(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get task metadata
    pub fn metadata(&self) -> &TaskMetadata {
        &self.metadata
    }

    /// Get task ID
    pub fn task_id(&self) -> u64 {
        self.task_id
    }
}

/// Snapshot of command queue statistics
#[derive(Debug, Clone)]
pub struct CommandQueueStatsSnapshot {
    pub total_commands: u64,
    pub queued_commands: usize,
    pub max_queue_depth: usize,
    pub avg_execution_time_ns: u64,
    pub throughput: u64,
}

/// Snapshot of coordinator statistics
#[derive(Debug, Clone)]
pub struct CoordinatorStatsSnapshot {
    pub total_tasks: u64,
    pub active_task_count: usize,
    pub completion_rate: u64,
    pub avg_task_duration_ns: u64,
    pub success_rate_percent: f64,
    pub command_queue_stats: CommandQueueStatsSnapshot,
}

impl CommandQueueStats {
    fn new() -> Self {
        Self {
            total_commands: AtomicU64::new(0),
            queued_commands: AtomicUsize::new(0),
            max_queue_depth: AtomicUsize::new(0),
            avg_execution_time: AtomicU64::new(0),
            throughput: AtomicU64::new(0),
        }
    }
}

impl CoordinatorStats {
    fn new() -> Self {
        Self {
            total_tasks: AtomicU64::new(0),
            active_task_count: AtomicUsize::new(0),
            completion_rate: AtomicU64::new(0),
            avg_task_duration: AtomicU64::new(0),
            successful_tasks: AtomicU64::new(0),
            completed_tasks: AtomicU64::new(0),
        }
    }

    fn update_task_completion(&self, duration: Duration, success: bool) {
        let duration_ns = duration.as_nanos() as u64;

        // Update average duration using exponential moving average
        let current_avg = self.avg_task_duration.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            duration_ns
        } else {
            (current_avg * 9 + duration_ns) / 10
        };
        self.avg_task_duration.store(new_avg, Ordering::Relaxed);

        // Update completion and success counters
        self.completed_tasks.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_tasks.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Calculate success rate as percentage (0.0-100.0)
    fn success_rate_percent(&self) -> f64 {
        let completed = self.completed_tasks.load(Ordering::Relaxed);
        let successful = self.successful_tasks.load(Ordering::Relaxed);

        if completed == 0 {
            0.0
        } else {
            (successful as f64 / completed as f64) * 100.0
        }
    }
}

impl Clone for CoordinatorStats {
    fn clone(&self) -> Self {
        Self {
            total_tasks: AtomicU64::new(self.total_tasks.load(Ordering::Relaxed)),
            active_task_count: AtomicUsize::new(self.active_task_count.load(Ordering::Relaxed)),
            completion_rate: AtomicU64::new(self.completion_rate.load(Ordering::Relaxed)),
            avg_task_duration: AtomicU64::new(self.avg_task_duration.load(Ordering::Relaxed)),
            successful_tasks: AtomicU64::new(self.successful_tasks.load(Ordering::Relaxed)),
            completed_tasks: AtomicU64::new(self.completed_tasks.load(Ordering::Relaxed)),
        }
    }
}

/// Task result tracker for operation result coordination using crossbeam channels
#[derive(Debug)]
pub struct TaskResultTracker<T> {
    /// Result channels keyed by task ID with timestamps
    result_channels: DashMap<
        u64,
        (
            crossbeam_channel::Receiver<Result<T, CacheOperationError>>,
            Instant,
        ),
    >,
    /// Channel cleanup tracker
    next_cleanup_id: AtomicU64,
}

impl<T> TaskResultTracker<T> {
    /// Create new task result tracker
    pub fn new() -> Self {
        Self {
            result_channels: DashMap::new(),
            next_cleanup_id: AtomicU64::new(1),
        }
    }

    /// Register a result channel for a task
    pub fn register_task_result(
        &self,
        task_id: u64,
    ) -> crossbeam_channel::Sender<Result<T, CacheOperationError>> {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.result_channels
            .insert(task_id, (receiver, Instant::now()));
        sender
    }

    /// Wait for task result with timeout
    pub fn wait_for_result(
        &self,
        task_id: u64,
        timeout: Duration,
    ) -> Result<T, CacheOperationError> {
        let (receiver, _timestamp) = self
            .result_channels
            .remove(&task_id)
            .ok_or_else(|| {
                CacheOperationError::InvalidState(format!(
                    "Task {} not found in result tracker",
                    task_id
                ))
            })?
            .1; // Get the value from the (key, value) tuple

        match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                Err(CacheOperationError::TimeoutError)
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Err(
                CacheOperationError::InvalidState("Task result channel disconnected".to_string()),
            ),
        }
    }

    /// Cleanup expired result channels based on actual time
    pub fn cleanup_expired_results(&self, max_age: Duration) -> usize {
        let now = Instant::now();
        let mut cleaned = 0;

        self.result_channels.retain(|&task_id, (_, timestamp)| {
            let age = now.duration_since(*timestamp);
            if age > max_age {
                cleaned += 1;
                log::debug!(
                    "Cleaning up expired task result channel for task_id: {} (age: {:?})",
                    task_id,
                    age
                );
                false // Remove this entry
            } else {
                true // Keep this entry
            }
        });

        if cleaned > 0 {
            log::info!("Cleaned up {} expired result channels", cleaned);
        }

        cleaned
    }

    /// Get number of pending results
    pub fn pending_results_count(&self) -> usize {
        self.result_channels.len()
    }

    /// Spawn a background cleanup thread with configurable interval and shutdown channel
    pub fn spawn_cleanup_thread(
        &self,
        cleanup_interval: Duration,
        max_age: Duration,
    ) -> Result<(std::thread::JoinHandle<()>, crossbeam_channel::Sender<()>), CacheOperationError>
    where
        T: Send + Sync + 'static,
    {
        // Create a weak reference to avoid circular dependencies
        let result_channels = self.result_channels.clone();

        // Create shutdown channel
        let (shutdown_sender, shutdown_receiver) = crossbeam_channel::bounded(1);

        let join_handle = std::thread::Builder::new()
            .name("task-result-cleanup".to_string())
            .spawn(move || {
                log::info!("TaskResultTracker cleanup thread started with interval {:?}, max_age {:?}", cleanup_interval, max_age);
                loop {
                    // Check for shutdown signal with timeout
                    match shutdown_receiver.recv_timeout(cleanup_interval) {
                        Ok(_) => {
                            log::info!("TaskResultTracker cleanup thread received shutdown signal");
                            break;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            // Continue with cleanup
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            log::warn!("TaskResultTracker cleanup shutdown channel disconnected");
                            break;
                        }
                    }
                    let now = Instant::now();
                    let mut cleaned = 0;

                    result_channels.retain(|&task_id, (_, timestamp)| {
                        let age = now.duration_since(*timestamp);
                        if age > max_age {
                            cleaned += 1;
                            log::debug!("Background cleanup removing expired task result channel for task_id: {} (age: {:?})", task_id, age);
                            false
                        } else {
                            true
                        }
                    });
                    if cleaned > 0 {
                        log::debug!("Background cleanup removed {} expired result channels", cleaned);
                    }
                }
                log::info!("TaskResultTracker cleanup thread shutting down");
            })
            .map_err(|e| CacheOperationError::initialization_failed(format!("Failed to spawn task result cleanup thread: {}", e)))?;

        Ok((join_handle, shutdown_sender))
    }
}
