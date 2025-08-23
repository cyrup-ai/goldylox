//! Task coordination and command queue system for cache operations
//!
//! This module provides safe coordination between async tasks and cache state,
//! implementing patterns from bevy's CommandQueue for deferred mutations.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::CacheTier;
use crate::cache::coordinator::background_coordinator::BackgroundCoordinator;
use crate::cache::manager::background::types::{BackgroundTask, MaintenanceTask};

/// Command queue for safe cache mutations from async contexts
#[derive(Debug)]
pub struct CacheCommandQueue<K: CacheKey, V: CacheValue> {
    /// Pending commands to execute
    commands: Mutex<VecDeque<CacheCommand<K, V>>>,
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

/// Task coordinator for managing cache operations
#[derive(Debug)]
pub struct TaskCoordinator<K: CacheKey, V: CacheValue> {
    /// Background coordinator for task processing
    background_coordinator: Arc<BackgroundCoordinator<K, V>>,
    /// Command queue for safe mutations
    command_queue: Arc<CacheCommandQueue<K, V>>,
    /// Active task tracking
    active_tasks: DashMap<u64, TaskInfo>,
    /// Task ID generator
    next_task_id: AtomicU64,
    /// Coordination statistics
    stats: CoordinatorStats,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
}

/// Information about active tasks
#[derive(Debug, Clone)]
pub struct TaskInfo {
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
    /// Associated cache keys
    keys: Vec<String>, // Serialized keys for tracking
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
    /// Task success rate
    success_rate: AtomicU64, // Percentage * 100
}

/// Task execution context for cache operations
#[derive(Debug)]
pub struct TaskExecutionContext<K: CacheKey, V: CacheValue> {
    /// Command queue for mutations
    command_queue: Arc<CacheCommandQueue<K, V>>,
    /// Task ID for tracking
    task_id: u64,
    /// Execution start time
    start_time: Instant,
    /// Task metadata
    metadata: TaskMetadata,
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
        Self {
            commands: Mutex::new(VecDeque::with_capacity(max_queue_size)),
            stats: CommandQueueStats::new(),
            max_queue_size,
        }
    }

    /// Enqueue command for deferred execution
    pub fn enqueue_command(&self, command: CacheCommand<K, V>) -> Result<(), CacheOperationError> {
        let mut commands = self
            .commands
            .lock()
            .map_err(|_| CacheOperationError::OperationFailed)?;

        // Check queue capacity
        if commands.len() >= self.max_queue_size {
            return Err(CacheOperationError::ResourceExhausted(
                "Command queue full".to_string(),
            ));
        }

        commands.push_back(command);
        self.stats.queued_commands.fetch_add(1, Ordering::Relaxed);

        // Update max queue depth
        let current_depth = commands.len();
        self.stats
            .max_queue_depth
            .fetch_max(current_depth, Ordering::Relaxed);

        Ok(())
    }

    /// Drain and execute all pending commands
    pub fn execute_pending_commands<F>(&self, mut executor: F) -> Result<usize, CacheOperationError>
    where
        F: FnMut(CacheCommand<K, V>) -> Result<(), CacheOperationError>,
    {
        let mut commands = self
            .commands
            .lock()
            .map_err(|_| CacheOperationError::OperationFailed)?;

        let command_count = commands.len();
        let execution_start = Instant::now();

        while let Some(command) = commands.pop_front() {
            let command_start = Instant::now();

            // Execute command
            if let Err(e) = executor(command) {
                // Log error but continue processing other commands
                eprintln!("Command execution failed: {:?}", e);
            }

            // Update statistics
            let execution_time = command_start.elapsed();
            self.update_execution_stats(execution_time);
        }

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
    pub fn clear(&self) -> Result<usize, CacheOperationError> {
        let mut commands = self
            .commands
            .lock()
            .map_err(|_| CacheOperationError::OperationFailed)?;

        let cleared_count = commands.len();
        commands.clear();
        self.stats.queued_commands.store(0, Ordering::Relaxed);

        Ok(cleared_count)
    }
}

impl<K: CacheKey, V: CacheValue> TaskCoordinator<K, V> {
    /// Create new task coordinator
    pub fn new(background_coordinator: Arc<BackgroundCoordinator<K, V>>, max_command_queue_size: usize) -> Self {
        let command_queue = Arc::new(CacheCommandQueue::new(max_command_queue_size));

        Self {
            background_coordinator,
            command_queue,
            active_tasks: DashMap::new(),
            next_task_id: AtomicU64::new(1),
            stats: CoordinatorStats::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Schedule cache operation task
    pub async fn schedule_cache_operation<F, T>(
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

        // Create task info
        let task_info = TaskInfo {
            id: task_id,
            task_type: task_type.clone(),
            priority,
            started_at: start_time,
            estimated_completion: None,
            keys: keys
                .iter()
                .enumerate()
                .map(|(i, _)| format!("key_{}", i))
                .collect(),
        };

        // Track active task
        self.active_tasks.insert(task_id, task_info);
        self.stats.active_task_count.fetch_add(1, Ordering::Relaxed);

        // Create execution context
        let context = TaskExecutionContext {
            command_queue: self.command_queue.clone(),
            task_id,
            start_time,
            metadata: TaskMetadata {
                task_type,
                priority,
                expected_duration: Duration::from_millis(100), // Default estimate
                retry_count: 0,
                numa_node: None,
            },
        };

        // Schedule task
        let active_tasks = self.active_tasks.clone();
        let stats = self.stats.clone();

        let future = async move {
            let result = operation(context);

            // Update statistics
            let task_duration = start_time.elapsed();
            stats.update_task_completion(task_duration, result.is_ok());

            // Remove from active tasks
            active_tasks.remove(&task_id);
            stats.active_task_count.fetch_sub(1, Ordering::Relaxed);

            result
        };

        // Submit background task using BackgroundCoordinator
        let background_task = BackgroundTask::Statistics {
            stats_type: 1,
            interval_ms: 100,
        };
        self.background_coordinator.submit_task(background_task)?;

        self.stats.total_tasks.fetch_add(1, Ordering::Relaxed);

        Ok(task_id)
    }

    /// Execute all pending commands
    pub fn flush_command_queue<F>(&self, executor: F) -> Result<usize, CacheOperationError>
    where
        F: FnMut(CacheCommand<K, V>) -> Result<(), CacheOperationError>,
    {
        self.command_queue.execute_pending_commands(executor)
    }

    /// Get coordinator statistics
    pub fn get_stats(&self) -> CoordinatorStatsSnapshot {
        CoordinatorStatsSnapshot {
            total_tasks: self.stats.total_tasks.load(Ordering::Relaxed),
            active_task_count: self.stats.active_task_count.load(Ordering::Relaxed),
            completion_rate: self.stats.completion_rate.load(Ordering::Relaxed),
            avg_task_duration_ns: self.stats.avg_task_duration.load(Ordering::Relaxed),
            success_rate_percent: self.stats.success_rate.load(Ordering::Relaxed) as f64 / 100.0,
            command_queue_stats: self.command_queue.get_stats(),
        }
    }

    /// Get active task information
    pub fn get_active_tasks(&self) -> Vec<TaskInfo> {
        self.active_tasks
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Cancel active task
    pub fn cancel_task(&self, task_id: u64) -> Result<bool, CacheOperationError> {
        // Remove from active tasks
        let was_active = self.active_tasks.remove(&task_id).is_some();

        if was_active {
            self.stats.active_task_count.fetch_sub(1, Ordering::Relaxed);
        }

        Ok(was_active)
    }

    /// Shutdown coordinator gracefully
    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        self.shutdown.store(true, Ordering::Relaxed);

        // Clear command queue
        let _cleared = self.command_queue.clear()?;

        // Cancel all active tasks
        let active_task_ids: Vec<_> = self.active_tasks.iter().map(|entry| *entry.key()).collect();
        for task_id in active_task_ids {
            let _ = self.cancel_task(task_id);
        }

        Ok(())
    }
}

impl<K: CacheKey, V: CacheValue> TaskExecutionContext<K, V> {
    /// Enqueue command for deferred execution
    pub fn enqueue_command(&self, command: CacheCommand<K, V>) -> Result<(), CacheOperationError> {
        self.command_queue.enqueue_command(command)
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
            success_rate: AtomicU64::new(0),
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

        // Update success rate
        if success {
            self.success_rate.fetch_add(1, Ordering::Relaxed);
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
            success_rate: AtomicU64::new(self.success_rate.load(Ordering::Relaxed)),
        }
    }
}
