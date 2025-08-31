//! Background operation types and coordination structures
//!
//! This module defines the fundamental types used throughout the background
//! operation system including coordinators, workers, and task definitions.

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64};
use std::time::Instant;

// Removed unused imports - using full qualification instead
use crossbeam_utils::atomic::AtomicCell;
use crate::cache::traits::types_and_enums::CacheOperationError;

use crate::cache::traits::{CacheKey, CacheValue};

/// Trait for processing background tasks
pub trait TaskProcessor: Send + Sync {
    /// Process a background task
    fn process_task(&self, task: &BackgroundTask) -> Result<(), CacheOperationError>;
}

/// Background task generic wrapper for type-erased operations
#[derive(Debug, Clone)]
pub enum BackgroundTask {
    /// Eviction task
    Eviction {
        /// Target tier for eviction
        tier: u8,
        /// Number of entries to evict
        count: u32,
        /// Priority level
        priority: u16,
    },
    /// Compression task
    Compression {
        /// Compression algorithm
        algorithm: u8,
        /// Target compression ratio
        ratio: f32,
    },
    /// Statistics collection task
    Statistics {
        /// Statistics type
        stats_type: u8,
        /// Collection interval
        interval_ms: u64,
    },
    /// Maintenance task
    Maintenance(MaintenanceTask),
    /// Prefetch task
    Prefetch {
        /// Number of entries to prefetch
        count: u32,
        /// Prefetch strategy
        strategy: u8,
    },
}

/// Background task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// Critical system tasks
    Critical = 0,
    /// High priority tasks
    High = 1,
    /// Normal priority tasks
    Normal = 2,
    /// Low priority background tasks
    Low = 3,
}

/// Background task execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently executing
    Executing,
    /// Task completed successfully
    Completed,
    /// Task failed with error
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Statistics collection types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticsType {
    /// Basic hit/miss statistics
    Basic,
    /// Detailed performance metrics
    Detailed,
    /// Comprehensive statistics with analysis
    Comprehensive,
}

// Re-export canonical MaintenanceTask enum for consistency
pub use crate::cache::tier::warm::maintenance::MaintenanceTask as CanonicalMaintenanceTask;

/// Maintenance task structure with scheduling metadata for background coordinator
#[derive(Debug, Clone)]
pub struct MaintenanceTask {
    /// Canonical task definition (unified across all cache subsystems)
    pub task: CanonicalMaintenanceTask,
    /// Task priority (lower = higher priority) - overrides canonical priority if needed
    pub priority: u16,
    /// Task creation timestamp
    pub created_at: Instant,
    /// Task timeout in nanoseconds
    pub timeout_ns: u64,
    /// Current retry count
    pub retry_count: u8,
    /// Maximum retry attempts
    pub max_retries: u8,
}

impl MaintenanceTask {
    /// Create a new maintenance task with canonical task definition
    pub fn new(task: CanonicalMaintenanceTask) -> Self {
        let priority = match task.priority() {
            crate::cache::tier::warm::maintenance::TaskPriority::High => 10,
            crate::cache::tier::warm::maintenance::TaskPriority::Medium => 50,
            crate::cache::tier::warm::maintenance::TaskPriority::Low => 100,
        };

        let timeout_ns = task.estimated_duration().as_nanos() as u64 * 10; // 10x estimated duration as timeout
        Self {
            task,
            priority,
            created_at: std::time::Instant::now(),
            timeout_ns,
            retry_count: 0,
            max_retries: 3,
        }
    }

    /// Create task with custom priority override
    pub fn with_priority(task: CanonicalMaintenanceTask, priority: u16) -> Self {
        let timeout_ns = task.estimated_duration().as_nanos() as u64 * 10;
        Self {
            task,
            priority,
            created_at: std::time::Instant::now(),
            timeout_ns,
            retry_count: 0,
            max_retries: 3,
        }
    }

    /// Create task with custom timeout
    pub fn with_timeout(task: CanonicalMaintenanceTask, timeout_ns: u64) -> Self {
        let priority = match task.priority() {
            crate::cache::tier::warm::maintenance::TaskPriority::High => 10,
            crate::cache::tier::warm::maintenance::TaskPriority::Medium => 50,
            crate::cache::tier::warm::maintenance::TaskPriority::Low => 100,
        };

        Self {
            task,
            priority,
            created_at: std::time::Instant::now(),
            timeout_ns,
            retry_count: 0,
            max_retries: 3,
        }
    }

    /// Check if task has timed out
    pub fn is_timed_out(&self) -> bool {
        let elapsed = self.created_at.elapsed().as_nanos() as u64;
        elapsed > self.timeout_ns
    }

    /// Check if task can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}



/// Maintenance operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceOperation {
    /// Cleanup expired entries
    CleanupExpired,
    /// Defragment storage
    Defragment,
    /// Rebuild indices
    RebuildIndices,
    /// Validate data integrity
    ValidateIntegrity,
    /// Optimize memory layout
    OptimizeMemory,
    /// Update access patterns
    UpdatePatterns,
}

/// Maintenance statistics tracking
#[derive(Debug, Default)]
pub struct MaintenanceStats {
    /// Total maintenance tasks submitted
    pub total_submitted: AtomicU64,
    /// Total maintenance operations executed
    pub operations_executed: AtomicU64,
    /// Total time spent on maintenance (nanoseconds)
    pub total_maintenance_time_ns: AtomicU64,
    /// Operations by type counters
    pub cleanup_operations: AtomicU64,
    pub defrag_operations: AtomicU64,
    pub rebuild_operations: AtomicU64,
    pub validation_operations: AtomicU64,
    pub optimization_operations: AtomicU64,
    pub pattern_operations: AtomicU64,
    /// Error counters
    pub failed_operations: AtomicU64,
    /// Last maintenance timestamp
    pub last_maintenance: AtomicCell<Option<Instant>>,
}

/// Synchronization operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOperation {
    /// Sync to persistent storage
    ToPersistent,
    /// Sync from persistent storage
    FromPersistent,
    /// Sync between cache tiers
    BetweenTiers,
    /// Sync with remote cache
    WithRemote,
}

/// Maintenance scheduler with atomic coordination and worker thread pool
#[derive(Debug)]
pub struct MaintenanceScheduler<K: CacheKey + Default, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static> {
    /// Maintenance interval in nanoseconds
    pub maintenance_interval_ns: u64,
    /// Last maintenance timestamp
    pub last_maintenance: std::time::Instant,
    /// Maintenance statistics with atomic tracking
    pub maintenance_stats: MaintenanceStats,
    /// Scheduled operations queue
    pub scheduled_operations: Vec<MaintenanceOperation>,

    /// Task queue for distributing maintenance tasks
    pub task_queue: crossbeam_channel::Receiver<MaintenanceTask>,
    /// Task sender for submitting maintenance tasks
    pub task_sender: crossbeam_channel::Sender<MaintenanceTask>,
    /// Worker thread handles for maintenance operations
    pub worker_threads: Vec<std::thread::JoinHandle<()>>,
    /// Scheduler configuration
    pub config: MaintenanceConfig,
    /// Maintenance statistics with atomic tracking
    pub stats: MaintenanceStats,
    /// Shutdown signal channel
    pub shutdown_signal: crossbeam_channel::Receiver<()>,
    /// Shutdown sender for graceful shutdown
    pub shutdown_sender: crossbeam_channel::Sender<()>,
    /// Scaling request receiver for dynamic worker management
    pub scaling_request_receiver: crossbeam_channel::Receiver<ScalingRequest>,
    /// Scaling request sender for external coordination
    pub scaling_request_sender: crossbeam_channel::Sender<ScalingRequest>,
    /// Phantom data for generic parameters
    pub _phantom: std::marker::PhantomData<(K, V)>,
}

/// Background worker state with work-stealing coordination
#[derive(Debug)]
pub struct BackgroundWorkerState {
    /// Worker identification
    pub worker_id: u32,
    /// Task processing statistics
    pub tasks_processed: AtomicU64,
    /// Processing time accumulator
    pub total_processing_time: AtomicU64,
    /// Worker status
    pub status: AtomicCell<WorkerStatus>,
    /// Last heartbeat timestamp
    pub last_heartbeat: AtomicCell<Instant>,
    /// Current task being processed (stored as discriminant for atomic access)
    pub current_task_discriminant: AtomicU8,
    /// Error counter
    pub error_count: AtomicU32,
    /// Work-stealing attempt counter
    pub steal_attempts: AtomicU64,
    /// Successful steals counter
    pub successful_steals: AtomicU64,
    /// Number of tasks processed since last heartbeat
    pub tasks_since_heartbeat: AtomicU64,
}

// BackgroundWorkerState implementation moved to worker_state.rs module
// This provides better separation of concerns and avoids duplicate methods

/// Maintenance scheduler configuration
#[derive(Debug, Clone)]
pub struct MaintenanceConfig {
    /// Number of worker threads
    pub worker_count: u32,
    /// Task queue capacity
    pub queue_capacity: usize,
    /// Worker heartbeat interval (nanoseconds)
    pub heartbeat_interval_ns: u64,
    /// Task timeout threshold (nanoseconds)
    pub task_timeout_ns: u64,
    /// Maximum retry attempts for failed tasks
    pub max_task_retries: u8,
    /// Work stealing enabled
    pub enable_work_stealing: bool,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            worker_count: 4,
            queue_capacity: 1000,
            heartbeat_interval_ns: 1_000_000_000, // 1 second
            task_timeout_ns: 30_000_000_000,      // 30 seconds
            max_task_retries: 3,
            enable_work_stealing: true,
        }
    }
}

/// Worker status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    /// Worker is idle and available for tasks
    Idle,
    /// Worker is processing a task
    Processing,
    /// Worker is attempting to steal work
    StealingWork,
    /// Worker encountered an error
    Error,
    /// Worker is shutting down
    Shutdown,
}

/// Scaling request for dynamic worker thread management
#[derive(Debug, Clone)]
pub struct ScalingRequest {
    /// Capacity factor (0.0 to 2.0) - 1.0 = current capacity, 0.5 = half, 2.0 = double
    pub capacity_factor: f64,
    /// Response channel for scaling operation result
    pub response_sender: crossbeam_channel::Sender<Result<(), String>>,
}

/// Global scaling request channel - initialized lazily
static GLOBAL_SCALING_SENDER: std::sync::OnceLock<crossbeam_channel::Sender<ScalingRequest>> = std::sync::OnceLock::new();

impl WorkerStatus {
    /// Request worker scaling through global coordination channel
    /// 
    /// # Arguments
    /// * `capacity_factor` - Scaling factor (0.75 = reduce to 75%, 1.5 = increase to 150%)
    /// 
    /// # Returns
    /// * `Ok(())` if scaling request was processed successfully
    /// * `Err(String)` if scaling failed or timed out
    pub fn request_worker_scaling(capacity_factor: f64) -> Result<(), String> {
        // Validate capacity factor
        if capacity_factor <= 0.0 || capacity_factor > 10.0 {
            return Err("Invalid capacity factor: must be between 0.0 and 10.0".to_string());
        }

        // Get or initialize the global scaling sender
        let scaling_sender = GLOBAL_SCALING_SENDER.get().ok_or_else(|| {
            "Worker scaling system not initialized. MaintenanceScheduler must be created first.".to_string()
        })?;

        // Create response channel for this scaling request
        let (response_sender, response_receiver) = crossbeam_channel::bounded::<Result<(), String>>(1);
        
        let scaling_request = ScalingRequest {
            capacity_factor,
            response_sender,
        };

        // Send scaling request
        scaling_sender.try_send(scaling_request).map_err(|e| {
            format!("Failed to send scaling request: {:?}", e)
        })?;

        // Wait for response with timeout
        match response_receiver.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(result) => result,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                Err("Scaling request timed out after 5 seconds".to_string())
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                Err("Scaling coordination channel disconnected".to_string())
            }
        }
    }

    /// Initialize global scaling coordination (called by MaintenanceScheduler)
    pub(crate) fn initialize_scaling_coordination(sender: crossbeam_channel::Sender<ScalingRequest>) -> Result<(), String> {
        GLOBAL_SCALING_SENDER.set(sender).map_err(|_| {
            "Scaling coordination already initialized".to_string()
        })
    }
}


