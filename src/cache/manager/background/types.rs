//! Background operation types and coordination structures
//!
//! This module defines the fundamental types used throughout the background
//! operation system including coordinators, workers, and task definitions.

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64};
use std::time::Instant;

// Removed unused imports - using full qualification instead
use crate::cache::traits::types_and_enums::CacheOperationError;
use crossbeam_utils::atomic::AtomicCell;

use crate::cache::traits::{CacheKey, CacheValue};

/// Trait for processing background tasks
#[allow(dead_code)] // Background task system - used in sophisticated task coordination
pub trait TaskProcessor: Send + Sync {
    /// Process a background task
    fn process_task(&self, task: &BackgroundTask) -> Result<(), CacheOperationError>;
}

/// Background task generic wrapper for type-erased operations
#[allow(dead_code)] // Background task system - used in sophisticated task coordination
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
        #[allow(dead_code)] // Background task field for compression ratio tuning
        ratio: f32,
    },
    /// Statistics collection task
    Statistics {
        /// Statistics type
        stats_type: u8,
        /// Collection interval
        #[allow(dead_code)] // Background task field for statistics interval configuration
        interval_ms: u64,
    },
    /// Maintenance task
    #[allow(dead_code)] // Background task variant for maintenance operations
    Maintenance(MaintenanceTask),
    /// Prefetch task
    Prefetch {
        /// Number of entries to prefetch
        #[allow(dead_code)] // Background task field for prefetch count configuration
        count: u32,
        /// Prefetch strategy
        strategy: u8,
    },
}

/// Background task priority levels
#[allow(dead_code)] // Background workers - task priority system used in coordinated background task processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// Critical system tasks
    #[allow(dead_code)]
    // Background workers - critical task priority used in emergency operations
    Critical = 0,
    /// High priority tasks
    #[allow(dead_code)]
    // Background workers - high task priority used in time-sensitive operations
    High = 1,
    /// Normal priority tasks
    #[allow(dead_code)]
    // Background workers - normal task priority used in standard background processing
    Normal = 2,
    /// Low priority background tasks
    #[allow(dead_code)]
    // Background workers - low task priority used in deferred maintenance operations
    Low = 3,
}

/// Background task execution status
#[allow(dead_code)] // Background workers - task status system used in background task lifecycle tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution
    #[allow(dead_code)] // Background workers - pending status used in task queue management
    Pending,
    /// Task is currently executing
    #[allow(dead_code)] // Background workers - executing status used in active task tracking
    Executing,
    /// Task completed successfully
    #[allow(dead_code)]
    // Background workers - completed status used in task completion tracking
    Completed,
    /// Task failed with error
    #[allow(dead_code)]
    // Background workers - failed status used in error recovery and retry logic
    Failed,
    /// Task was cancelled
    #[allow(dead_code)]
    // Background workers - cancelled status used in graceful shutdown and task cancellation
    Cancelled,
}

/// Statistics collection types
#[allow(dead_code)] // Background workers - statistics type classification used in performance monitoring systems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticsType {
    /// Basic hit/miss statistics
    #[allow(dead_code)] // Background workers - basic stats used in lightweight monitoring
    Basic,
    /// Detailed performance metrics
    #[allow(dead_code)] // Background workers - detailed stats used in performance analysis
    Detailed,
    /// Comprehensive statistics with analysis
    #[allow(dead_code)]
    // Background workers - comprehensive stats used in deep system analysis
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
    #[allow(dead_code)]
    // Background workers - created_at used in task timeout and age calculation
    pub created_at: Instant,
    /// Task timeout in nanoseconds
    #[allow(dead_code)] // Background workers - timeout_ns used in task timeout enforcement
    pub timeout_ns: u64,
    /// Current retry count
    #[allow(dead_code)]
    // Background workers - retry_count used in retry logic and failure handling
    pub retry_count: u8,
    /// Maximum retry attempts
    #[allow(dead_code)] // Background workers - max_retries used in retry policy enforcement
    pub max_retries: u8,
}

impl MaintenanceTask {
    /// Create a new maintenance task with canonical task definition
    #[allow(dead_code)] // Background types - maintenance task constructor for task creation
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
            max_retries: 3, // Default, can be overridden with new_with_config
        }
    }

    /// Create a new maintenance task with configuration
    pub fn new_with_config(task: CanonicalMaintenanceTask, config: &MaintenanceConfig) -> Self {
        let priority = match task.priority() {
            crate::cache::tier::warm::maintenance::TaskPriority::High => 10,
            crate::cache::tier::warm::maintenance::TaskPriority::Medium => 50,
            crate::cache::tier::warm::maintenance::TaskPriority::Low => 100,
        };

        // Use configured timeout and max retries from MaintenanceConfig
        let timeout_ns = if config.task_timeout_ns > 0 {
            config.task_timeout_ns
        } else {
            task.estimated_duration().as_nanos() as u64 * 10 // Fallback to 10x estimated duration
        };

        Self {
            task,
            priority,
            created_at: std::time::Instant::now(),
            timeout_ns,
            retry_count: 0,
            max_retries: config.max_task_retries,
        }
    }

    /// Create task with custom priority override
    #[allow(dead_code)] // Background types - maintenance task constructor with priority customization
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
    #[allow(dead_code)] // Background workers - with_timeout used in custom timeout task creation
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
    #[allow(dead_code)] // Background workers - is_timed_out used in task timeout detection and cleanup
    pub fn is_timed_out(&self) -> bool {
        let elapsed = self.created_at.elapsed().as_nanos() as u64;
        elapsed > self.timeout_ns
    }

    /// Check if task can be retried
    #[allow(dead_code)] // Background workers - can_retry used in retry decision making and error recovery
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count
    #[allow(dead_code)] // Background workers - increment_retry used in retry count management and failure tracking
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// Maintenance operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceOperation {
    /// Cleanup expired entries
    #[allow(dead_code)]
    // Background workers - cleanup expired used in periodic maintenance operations
    CleanupExpired,
    /// Defragment storage
    #[allow(dead_code)]
    // Background workers - defragment used in storage optimization operations
    Defragment,
    /// Rebuild indices
    #[allow(dead_code)]
    // Background workers - rebuild indices used in index maintenance operations
    RebuildIndices,
    /// Validate data integrity
    #[allow(dead_code)]
    // Background workers - validate integrity used in data consistency checks
    ValidateIntegrity,
    /// Optimize memory layout
    #[allow(dead_code)]
    // Background workers - optimize memory used in memory management operations
    OptimizeMemory,
    /// Update access patterns
    #[allow(dead_code)]
    // Background workers - update patterns used in pattern analysis and optimization
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
#[allow(dead_code)] // Background workers - sync operations used in coordination tasks
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
#[allow(dead_code)] // Background workers - used in async task processing and worker coordination
#[derive(Debug)]
pub struct MaintenanceScheduler<
    K: CacheKey + Default,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> {
    /// Maintenance interval in nanoseconds
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub maintenance_interval_ns: u64,
    /// Last maintenance timestamp
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub last_maintenance: std::time::Instant,
    /// Maintenance statistics with atomic tracking
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub maintenance_stats: MaintenanceStats,
    /// Scheduled operations queue
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub scheduled_operations: Vec<MaintenanceOperation>,

    /// Task queue for distributing maintenance tasks
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub task_queue: crossbeam_channel::Receiver<MaintenanceTask>,
    /// Task sender for submitting maintenance tasks
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub task_sender: crossbeam_channel::Sender<MaintenanceTask>,
    /// Coordinator thread handle (owns worker threads via crossbeam messaging)
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub coordinator_handle: Option<std::thread::JoinHandle<()>>,
    /// Scheduler configuration
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub config: MaintenanceConfig,
    /// Maintenance statistics with atomic tracking
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub stats: MaintenanceStats,
    /// Shutdown signal channel
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub shutdown_signal: crossbeam_channel::Receiver<()>,
    /// Shutdown sender for graceful shutdown
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub shutdown_sender: crossbeam_channel::Sender<()>,
    /// Scaling request receiver for dynamic worker management
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub scaling_request_receiver: crossbeam_channel::Receiver<ScalingRequest>,
    /// Scaling request sender for external coordination
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub scaling_request_sender: crossbeam_channel::Sender<ScalingRequest>,
    /// Hot tier coordinator for tier operations
    #[allow(dead_code)]
    // Background workers - coordinator for hot tier operations
    pub hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
    /// Warm tier coordinator for tier operations
    #[allow(dead_code)]
    // Background workers - coordinator for warm tier operations
    pub warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
    /// Cold tier coordinator for tier operations
    #[allow(dead_code)]
    // Background workers - coordinator for cold tier operations
    pub cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    /// Per-instance worker registry for status tracking and monitoring
    pub worker_registry: std::sync::Arc<dashmap::DashMap<u32, super::worker_state::WorkerStatusChannel>>,
    /// Phantom data for generic parameters
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub _phantom: std::marker::PhantomData<(K, V)>,
}

/// Worker context containing shared dependencies for worker threads
/// Reduces parameter count in worker functions from 11 to 7
#[derive(Clone)]
pub struct WorkerContext {
    pub unified_stats: std::sync::Arc<crate::telemetry::unified_stats::UnifiedCacheStatistics>,
    pub coherence_stats: std::sync::Arc<crate::cache::coherence::statistics::core_statistics::CoherenceStatistics>,
    pub hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
    pub warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
    pub cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    pub worker_registry: std::sync::Arc<dashmap::DashMap<u32, super::worker_state::WorkerStatusChannel>>,
    /// Per-instance scaling request sender for dynamic worker management
    pub scaling_sender: crossbeam_channel::Sender<ScalingRequest>,
    /// Per-instance pool coordinator for cleanup operations
    pub pool_coordinator: std::sync::Arc<crate::cache::memory::pool_manager::cleanup_manager::PoolCoordinator>,
}

/// Background worker state with work-stealing coordination
#[allow(dead_code)] // Background workers - used in async task processing and worker coordination
#[derive(Debug)]
pub struct BackgroundWorkerState {
    /// Worker identification
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub worker_id: u32,
    /// Task processing statistics
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub tasks_processed: AtomicU64,
    /// Processing time accumulator
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub total_processing_time: AtomicU64,
    /// Worker status
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub status: AtomicCell<WorkerStatus>,
    /// Last heartbeat timestamp
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub last_heartbeat: AtomicCell<Instant>,
    /// Current task being processed (stored as discriminant for atomic access)
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub current_task_discriminant: AtomicU8,
    /// Error counter
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub error_count: AtomicU32,
    /// Work-stealing attempt counter
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub steal_attempts: AtomicU64,
    /// Successful steals counter
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub successful_steals: AtomicU64,
    /// Number of tasks processed since last heartbeat
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
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
    /// Work stealing configuration
    pub work_stealing_active: bool,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            worker_count: 4,
            queue_capacity: 1000,
            heartbeat_interval_ns: 1_000_000_000, // 1 second
            task_timeout_ns: 30_000_000_000,      // 30 seconds
            max_task_retries: 3,
            work_stealing_active: true,
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

impl WorkerStatus {
    /// Request worker scaling through instance-specific coordination channel
    ///
    /// # Arguments
    /// * `scaling_sender` - Instance-specific scaling sender from WorkerContext
    /// * `capacity_factor` - Scaling factor (0.75 = reduce to 75%, 1.5 = increase to 150%)
    ///
    /// # Returns
    /// * `Ok(())` if scaling request was processed successfully
    /// * `Err(String)` if scaling failed or timed out
    ///
    /// # Example
    /// ```rust
    /// // Inside a worker with access to WorkerContext:
    /// WorkerStatus::request_worker_scaling(&context.scaling_sender, 1.5)?;
    /// ```
    pub fn request_worker_scaling(
        scaling_sender: &crossbeam_channel::Sender<ScalingRequest>,
        capacity_factor: f64,
    ) -> Result<(), String> {
        // Validate capacity factor
        if capacity_factor <= 0.0 || capacity_factor > 10.0 {
            return Err("Invalid capacity factor: must be between 0.0 and 10.0".to_string());
        }

        // Create response channel for this scaling request
        let (response_sender, response_receiver) =
            crossbeam_channel::bounded::<Result<(), String>>(1);

        let scaling_request = ScalingRequest {
            capacity_factor,
            response_sender,
        };

        // Send scaling request to instance-specific channel
        scaling_sender
            .try_send(scaling_request)
            .map_err(|e| format!("Failed to send scaling request: {:?}", e))?;

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
}
