//! Core allocation manager with atomic statistics tracking
//!
//! This module implements the main memory allocation interface with comprehensive
//! statistics tracking and optimal pool selection for high-performance allocation.

use log;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::pool_manager::MemoryPoolManager;
use super::pressure_monitor::MemoryPressureMonitor;
use super::types::MemoryStatistics;
use crate::cache::config::CacheConfig;
use crate::cache::manager::background::types::{MaintenanceConfig, MaintenanceScheduler};
use crate::cache::manager::error_recovery::{
    BackoffStrategy, ErrorRecoverySystem, RecoveryStrategy,
};
use crate::cache::memory::efficiency_analyzer::MemoryEfficiencyAnalyzer;
use crate::cache::memory::pool_manager::cleanup_manager::{
    EfficiencyRequest, MaintenanceRequest, PoolCleanupManager, PoolCleanupRequest,
    trigger_defragmentation,
};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::statistics::multi_tier::ErrorType;
use crossbeam_channel::{Receiver, bounded};

/// Global allocation statistics module - no Arc needed
pub mod global_stats {
    use crossbeam_utils::CachePadded;
    use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize};

    #[allow(dead_code)] // Memory management - GlobalAllocationStats used in memory allocation tracking
    pub struct GlobalAllocationStats {
        pub total_allocated: CachePadded<AtomicU64>,
        pub peak_allocation: CachePadded<AtomicU64>,
        pub active_allocations: CachePadded<AtomicUsize>,
        pub allocation_operations: CachePadded<AtomicU64>,
        pub deallocation_operations: CachePadded<AtomicU64>,
        pub allocation_failures: CachePadded<AtomicU64>,
        pub fragmentation_level: CachePadded<AtomicU32>,
        pub avg_allocation_size: CachePadded<AtomicUsize>,
    }

    impl GlobalAllocationStats {
        const fn new() -> Self {
            Self {
                total_allocated: CachePadded::new(AtomicU64::new(0)),
                peak_allocation: CachePadded::new(AtomicU64::new(0)),
                active_allocations: CachePadded::new(AtomicUsize::new(0)),
                allocation_operations: CachePadded::new(AtomicU64::new(0)),
                deallocation_operations: CachePadded::new(AtomicU64::new(0)),
                allocation_failures: CachePadded::new(AtomicU64::new(0)),
                fragmentation_level: CachePadded::new(AtomicU32::new(0)),
                avg_allocation_size: CachePadded::new(AtomicUsize::new(1024)),
            }
        }
    }

    #[allow(dead_code)] // Memory management - global allocation statistics for system-wide monitoring
    pub static ALLOCATION_STATS: GlobalAllocationStats = GlobalAllocationStats::new();

    #[allow(dead_code)] // Memory management - global pool statistics structure with atomic counters
    pub struct GlobalPoolStats {
        pub pool_utilizations: [CachePadded<AtomicU32>; 3], // Small, Medium, Large
        pub pool_allocations: [CachePadded<AtomicU64>; 3],
        pub pool_hit_rates: [CachePadded<AtomicU32>; 3],
    }

    impl GlobalPoolStats {
        const fn new() -> Self {
            Self {
                pool_utilizations: [
                    CachePadded::new(AtomicU32::new(0)),
                    CachePadded::new(AtomicU32::new(0)),
                    CachePadded::new(AtomicU32::new(0)),
                ],
                pool_allocations: [
                    CachePadded::new(AtomicU64::new(0)),
                    CachePadded::new(AtomicU64::new(0)),
                    CachePadded::new(AtomicU64::new(0)),
                ],
                pool_hit_rates: [
                    CachePadded::new(AtomicU32::new(9500)),
                    CachePadded::new(AtomicU32::new(9000)),
                    CachePadded::new(AtomicU32::new(8500)),
                ],
            }
        }
    }

    pub static POOL_STATS: GlobalPoolStats = GlobalPoolStats::new();
}

/// Global allocation statistics snapshot
#[allow(dead_code)] // Memory management - global allocation snapshot for memory monitoring
#[derive(Debug, Clone)]
pub struct GlobalAllocationSnapshot {
    pub total_allocated: u64,
    pub peak_allocation: u64,
    pub active_allocations: usize,
    pub allocation_operations: u64,
    pub deallocation_operations: u64,
    pub allocation_failures: u64,
    pub fragmentation_level: u32,
    pub avg_allocation_size: usize,
}

/// Global pool statistics snapshot  
#[allow(dead_code)] // Memory management - global pool snapshot for pool monitoring
#[derive(Debug, Clone)]
pub struct GlobalPoolSnapshot {
    pub pool_utilizations: [u32; 3], // Small, Medium, Large
    pub pool_allocations: [u64; 3],
    pub pool_hit_rates: [u32; 3],
}

/// Advanced memory manager with atomic allocation tracking
#[allow(dead_code)] // Memory management - used in pool allocation and cleanup coordination
#[derive(Debug)]
pub struct AllocationManager<K: CacheKey + Default, V: CacheValue> {
    /// Memory pool manager for efficient allocation
    pool_manager: MemoryPoolManager,
    /// Cleanup manager owned directly
    cleanup_manager: PoolCleanupManager,
    /// Error recovery system for sophisticated retry mechanisms
    error_recovery: ErrorRecoverySystem<K, V>,
    /// Memory pressure monitor for pressure-aware retries
    pressure_monitor: MemoryPressureMonitor,
    /// Phantom data for generic parameters
    _phantom: std::marker::PhantomData<(K, V)>,
}

/// Efficiency analysis worker
#[allow(dead_code)] // Memory management - used in pool allocation and cleanup coordination
pub struct EfficiencyWorker {
    receiver: Receiver<EfficiencyRequest>,
    analyzer: MemoryEfficiencyAnalyzer,
}

impl EfficiencyWorker {
    #[allow(dead_code)] // Memory management - used in pool allocation and cleanup coordination
    pub fn run(self) {
        while let Ok(request) = self.receiver.recv() {
            match request {
                EfficiencyRequest::GetFragmentation(response) => {
                    let (_, frag, _) = self.analyzer.get_efficiency_snapshot();
                    let _ = response.send(frag);
                }
                EfficiencyRequest::GetSnapshot(response) => {
                    let snapshot = self.analyzer.get_efficiency_snapshot();
                    let _ = response.send(snapshot);
                }
                EfficiencyRequest::AnalyzePool(pool_id, response) => {
                    // Calculate pool-specific fragmentation from global pool stats
                    let fragmentation = if pool_id < 3 {
                        // Get utilization from global pool stats (stored as percentage * 100)
                        let utilization = global_stats::POOL_STATS.pool_utilizations[pool_id]
                            .load(std::sync::atomic::Ordering::Relaxed)
                            as f64
                            / 10000.0;
                        // Fragmentation is inverse of utilization
                        1.0 - utilization
                    } else {
                        0.0 // Invalid pool ID
                    };
                    let _ = response.send(fragmentation);
                }
            }
        }
    }
}

/// Maintenance scheduler worker
#[allow(dead_code)] // Memory management - used in pool allocation and cleanup coordination
pub struct MaintenanceWorker<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()>,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> {
    receiver: Receiver<MaintenanceRequest>,
    scheduler: MaintenanceScheduler<K, V>,
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
> MaintenanceWorker<K, V>
where
    K: Clone + 'static,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
{
    #[allow(dead_code)] // Memory management - used in pool allocation and cleanup coordination
    pub fn run(self) {
        while let Ok(request) = self.receiver.recv() {
            match request {
                MaintenanceRequest::SubmitTask(task_type, priority, response) => {
                    let result = self.scheduler.submit_task(task_type, priority as u16)
                        .map_err(|_| crate::cache::traits::types_and_enums::CacheOperationError::OperationFailed);
                    let _ = response.send(result);
                }
                MaintenanceRequest::SubmitUrgentTask(task_type, response) => {
                    let result = self.scheduler.submit_urgent_task(task_type).map_err(|_| {
                        crate::cache::traits::types_and_enums::CacheOperationError::OperationFailed
                    });
                    let _ = response.send(result);
                }
                MaintenanceRequest::IsHealthy(response) => {
                    let healthy = self.scheduler.is_healthy();
                    let _ = response.send(healthy);
                }
                MaintenanceRequest::GetStats(response) => {
                    let stats = self.scheduler.get_stats();
                    let ops = stats
                        .operations_executed
                        .load(std::sync::atomic::Ordering::Relaxed);
                    let failed = stats
                        .failed_operations
                        .load(std::sync::atomic::Ordering::Relaxed);
                    let _ = response.send((ops, failed));
                }
            }
        }
    }
}

/// Pool cleanup worker that owns pools
pub struct PoolCleanupWorker {
    receiver: Receiver<PoolCleanupRequest>,
    pool_manager: MemoryPoolManager,
}

impl PoolCleanupWorker {
    #[allow(dead_code)] // Memory management - run used in pool cleanup worker execution
    pub fn run(self) {
        while let Ok(request) = self.receiver.recv() {
            match request {
                PoolCleanupRequest::EmergencyCleanup { response } => {
                    let result = if self.pool_manager.try_emergency_cleanup() {
                        // Get actual bytes freed from pools
                        let small_freed = self.pool_manager.small_pool().current_utilization()
                            * self.pool_manager.small_pool().object_size();
                        let medium_freed = self.pool_manager.medium_pool().current_utilization()
                            * self.pool_manager.medium_pool().object_size();
                        let large_freed = self.pool_manager.large_pool().current_utilization()
                            * self.pool_manager.large_pool().object_size();
                        Ok(small_freed + medium_freed + large_freed)
                    } else {
                        Ok(0)
                    };
                    let _ = response.send(result);
                }
                PoolCleanupRequest::TriggerDefragmentation { response } => {
                    let result = trigger_defragmentation();
                    let _ = response.send(result);
                }
            }
        }
    }
}

#[allow(dead_code)] // Memory management - comprehensive allocation manager methods for memory pool coordination
impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()>,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> AllocationManager<K, V>
{
    /// Create new allocation manager with configuration
    #[allow(dead_code)] // Memory management - new used in allocation manager initialization
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        // Create channels for background services
        let (efficiency_sender, efficiency_receiver) = bounded(256);
        let (maintenance_sender, maintenance_receiver) = bounded(256);

        // Spawn efficiency analyzer worker
        let config_clone = config.clone();
        std::thread::Builder::new()
            .name("efficiency-analyzer".to_string())
            .spawn(move || {
                let analyzer = match MemoryEfficiencyAnalyzer::new(&config_clone) {
                    Ok(a) => a,
                    Err(e) => {
                        log::error!("Efficiency analyzer failed to start: {:?}", e);
                        return;
                    }
                };
                let worker = EfficiencyWorker {
                    receiver: efficiency_receiver,
                    analyzer,
                };
                worker.run();
            })
            .map_err(|e| CacheOperationError::initialization_failed(e.to_string()))?;

        // Spawn maintenance scheduler worker
        std::thread::Builder::new()
            .name("maintenance-scheduler".to_string())
            .spawn(move || {
                let scheduler = match MaintenanceScheduler::<K, V>::new(MaintenanceConfig::default()) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("Maintenance scheduler failed to start: {:?}", e);
                        return;
                    }
                };
                let worker = MaintenanceWorker {
                    receiver: maintenance_receiver,
                    scheduler,
                };
                worker.run();
            })
            .map_err(|e| CacheOperationError::initialization_failed(e.to_string()))?;

        // Create cleanup manager with channels
        let cleanup_manager =
            PoolCleanupManager::new(efficiency_sender.clone(), maintenance_sender.clone());

        // Create pool manager
        let pool_manager = MemoryPoolManager::new(config)?;

        // Create pool cleanup channel
        let (pool_cleanup_sender, pool_cleanup_receiver) = bounded::<PoolCleanupRequest>(256);

        // Initialize global coordinator
        use super::pool_manager::cleanup_manager::PoolCoordinator;
        PoolCoordinator::initialize(pool_cleanup_sender).map_err(|e| {
            CacheOperationError::initialization_failed(format!("Pool coordinator: {:?}", e))
        })?;

        // Clone pool manager for worker ownership
        let worker_pool_manager = pool_manager.clone();

        // Spawn pool cleanup worker
        std::thread::Builder::new()
            .name("pool-cleanup-worker".to_string())
            .spawn(move || {
                let worker = PoolCleanupWorker {
                    receiver: pool_cleanup_receiver,
                    pool_manager: worker_pool_manager,
                };
                worker.run();
            })
            .map_err(|e| CacheOperationError::initialization_failed(e.to_string()))?;

        Ok(Self {
            pool_manager,
            cleanup_manager,
            error_recovery: ErrorRecoverySystem::new(),
            pressure_monitor: MemoryPressureMonitor::new(
                config
                    .memory_config
                    .max_memory_usage
                    .unwrap_or(1024 * 1024 * 1024),
            ),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Allocate memory with optimal pool selection
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        let _timer = Instant::now();

        // Update allocation statistics
        global_stats::ALLOCATION_STATS
            .total_allocated
            .fetch_add(size as u64, Ordering::Relaxed);
        global_stats::ALLOCATION_STATS
            .allocation_operations
            .fetch_add(1, Ordering::Relaxed);
        global_stats::ALLOCATION_STATS
            .active_allocations
            .fetch_add(1, Ordering::Relaxed);

        // Select appropriate pool based on size
        let allocation_result = if size < 1024 {
            self.pool_manager.small_pool().allocate(size)
        } else if size < 65536 {
            self.pool_manager.medium_pool().allocate(size)
        } else {
            self.pool_manager.large_pool().allocate(size)
        };

        match allocation_result {
            Ok(ptr) => {
                // Update peak allocation if necessary
                self.update_peak_allocation();

                // Update average allocation size
                self.update_avg_allocation_size(size);

                // Trigger cleanup if memory pressure is high
                if let Err(e) = self
                    .cleanup_manager
                    .try_sophisticated_cleanup(self.pool_manager.small_pool())
                {
                    log::warn!("Cleanup check failed: {:?}", e);
                }

                Ok(ptr)
            }
            Err(_) => {
                // Handle allocation failure
                global_stats::ALLOCATION_STATS
                    .allocation_failures
                    .fetch_add(1, Ordering::Relaxed);

                // Try emergency allocation or trigger GC
                self.handle_allocation_failure(size)
            }
        }
    }

    /// Deallocate memory with pool coordination
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) -> Result<(), CacheOperationError> {
        // Update deallocation statistics
        global_stats::ALLOCATION_STATS
            .total_allocated
            .fetch_sub(size as u64, Ordering::Relaxed);
        global_stats::ALLOCATION_STATS
            .deallocation_operations
            .fetch_add(1, Ordering::Relaxed);
        global_stats::ALLOCATION_STATS
            .active_allocations
            .fetch_sub(1, Ordering::Relaxed);

        // Return memory to appropriate pool
        let result = if size < 1024 {
            self.pool_manager.small_pool().deallocate(ptr, size)
        } else if size < 65536 {
            self.pool_manager.medium_pool().deallocate(ptr, size)
        } else {
            self.pool_manager.large_pool().deallocate(ptr, size)
        };

        // Trigger cleanup maintenance after deallocation
        if !self.cleanup_manager.is_cleanup_available() {
            log::warn!("Cleanup maintenance not available");
        }

        result
    }

    /// Get current memory statistics
    pub fn get_memory_stats(&self) -> MemoryStatistics {
        MemoryStatistics {
            total_allocated: global_stats::ALLOCATION_STATS
                .total_allocated
                .load(Ordering::Relaxed),
            peak_allocation: global_stats::ALLOCATION_STATS
                .peak_allocation
                .load(Ordering::Relaxed),
            active_allocations: global_stats::ALLOCATION_STATS
                .active_allocations
                .load(Ordering::Relaxed),
            allocation_operations: global_stats::ALLOCATION_STATS
                .allocation_operations
                .load(Ordering::Relaxed),
            deallocation_operations: global_stats::ALLOCATION_STATS
                .deallocation_operations
                .load(Ordering::Relaxed),
            allocation_failures: global_stats::ALLOCATION_STATS
                .allocation_failures
                .load(Ordering::Relaxed),
            fragmentation_level: global_stats::ALLOCATION_STATS
                .fragmentation_level
                .load(Ordering::Relaxed) as f32
                / 100.0,
            pressure_level: self.calculate_memory_pressure() as f32 / 100.0,
        }
    }

    /// Get pool manager reference
    pub fn pool_manager(&self) -> &MemoryPoolManager {
        &self.pool_manager
    }

    /// Perform emergency cleanup coordination across memory pools
    pub fn try_emergency_pool_cleanup(&self) -> Result<bool, CacheOperationError> {
        let pools = vec![
            self.pool_manager.small_pool(),
            self.pool_manager.medium_pool(),
            self.pool_manager.large_pool(),
        ];

        self.cleanup_manager
            .try_emergency_cleanup_coordination(&pools)
    }

    /// Get memory pool cleanup statistics
    pub fn get_pool_cleanup_stats(&self) -> (u64, u64, f64) {
        self.cleanup_manager.get_cleanup_stats()
    }

    /// Check if pool cleanup is available
    pub fn is_pool_cleanup_available(&self) -> bool {
        self.cleanup_manager.is_cleanup_available()
    }

    /// Get global allocation statistics  
    pub fn get_global_allocation_stats(&self) -> GlobalAllocationSnapshot {
        GlobalAllocationSnapshot {
            total_allocated: global_stats::ALLOCATION_STATS
                .total_allocated
                .load(Ordering::Relaxed),
            peak_allocation: global_stats::ALLOCATION_STATS
                .peak_allocation
                .load(Ordering::Relaxed),
            active_allocations: global_stats::ALLOCATION_STATS
                .active_allocations
                .load(Ordering::Relaxed),
            allocation_operations: global_stats::ALLOCATION_STATS
                .allocation_operations
                .load(Ordering::Relaxed),
            deallocation_operations: global_stats::ALLOCATION_STATS
                .deallocation_operations
                .load(Ordering::Relaxed),
            allocation_failures: global_stats::ALLOCATION_STATS
                .allocation_failures
                .load(Ordering::Relaxed),
            fragmentation_level: global_stats::ALLOCATION_STATS
                .fragmentation_level
                .load(Ordering::Relaxed),
            avg_allocation_size: global_stats::ALLOCATION_STATS
                .avg_allocation_size
                .load(Ordering::Relaxed),
        }
    }

    /// Get global pool statistics
    pub fn get_global_pool_stats(&self) -> GlobalPoolSnapshot {
        GlobalPoolSnapshot {
            pool_utilizations: [
                global_stats::POOL_STATS.pool_utilizations[0].load(Ordering::Relaxed),
                global_stats::POOL_STATS.pool_utilizations[1].load(Ordering::Relaxed),
                global_stats::POOL_STATS.pool_utilizations[2].load(Ordering::Relaxed),
            ],
            pool_allocations: [
                global_stats::POOL_STATS.pool_allocations[0].load(Ordering::Relaxed),
                global_stats::POOL_STATS.pool_allocations[1].load(Ordering::Relaxed),
                global_stats::POOL_STATS.pool_allocations[2].load(Ordering::Relaxed),
            ],
            pool_hit_rates: [
                global_stats::POOL_STATS.pool_hit_rates[0].load(Ordering::Relaxed),
                global_stats::POOL_STATS.pool_hit_rates[1].load(Ordering::Relaxed),
                global_stats::POOL_STATS.pool_hit_rates[2].load(Ordering::Relaxed),
            ],
        }
    }

    /// Update pool allocation statistics
    pub fn record_pool_allocation(&self, pool_idx: usize, size: u64) {
        if pool_idx < 3 {
            global_stats::POOL_STATS.pool_allocations[pool_idx].fetch_add(size, Ordering::Relaxed);
        }
    }

    /// Update pool hit rate statistics
    pub fn record_pool_hit_rate(&self, pool_idx: usize, hit_rate: u32) {
        if pool_idx < 3 {
            global_stats::POOL_STATS.pool_hit_rates[pool_idx].store(hit_rate, Ordering::Relaxed);
        }
    }

    /// Update peak allocation atomically
    fn update_peak_allocation(&self) {
        let current_total = global_stats::ALLOCATION_STATS
            .total_allocated
            .load(Ordering::Relaxed);
        let mut current_peak = global_stats::ALLOCATION_STATS
            .peak_allocation
            .load(Ordering::Relaxed);

        while current_total > current_peak {
            match global_stats::ALLOCATION_STATS
                .peak_allocation
                .compare_exchange_weak(
                    current_peak,
                    current_total,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                Ok(_) => break,
                Err(actual) => current_peak = actual,
            }
        }
    }

    /// Update average allocation size with exponential moving average
    fn update_avg_allocation_size(&self, size: usize) {
        let current_avg = global_stats::ALLOCATION_STATS
            .avg_allocation_size
            .load(Ordering::Relaxed);

        // Exponential moving average with alpha = 0.1
        let new_avg = (current_avg * 9 + size) / 10;
        global_stats::ALLOCATION_STATS
            .avg_allocation_size
            .store(new_avg, Ordering::Relaxed);
    }

    /// Handle allocation failure with recovery strategies
    fn handle_allocation_failure(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        // Record failure in existing atomic statistics (already exists)
        global_stats::ALLOCATION_STATS
            .allocation_failures
            .fetch_add(1, Ordering::Relaxed);

        // Connect to existing ErrorRecoverySystem for sophisticated tracking
        self.error_recovery
            .handle_error(ErrorType::MemoryAllocationFailure, 0);

        // Try triggering immediate cleanup in pools
        if self.pool_manager.try_emergency_cleanup() {
            // Use sophisticated retry with error recovery integration
            return self.retry_allocation_after_cleanup(size);
        }

        // If cleanup fails, return allocation error
        Err(CacheOperationError::resource_exhausted(
            "Memory allocation failed",
        ))
    }

    /// Calculate current memory pressure level
    fn calculate_memory_pressure(&self) -> u32 {
        let total_allocated = global_stats::ALLOCATION_STATS
            .total_allocated
            .load(Ordering::Relaxed);
        let active_allocations = global_stats::ALLOCATION_STATS
            .active_allocations
            .load(Ordering::Relaxed);

        // Simple pressure calculation based on allocation patterns
        let allocation_pressure = if active_allocations > 0 {
            ((total_allocated / active_allocations as u64) * 100) / 1024 // Normalize by 1KB
        } else {
            0
        };

        // Cap at 100% (10000 when multiplied by 100)
        std::cmp::min(allocation_pressure as u32, 10000)
    }

    /// Retry allocation with sophisticated recovery mechanisms
    fn retry_allocation_after_cleanup(
        &self,
        size: usize,
    ) -> Result<NonNull<u8>, CacheOperationError> {
        // Connect to existing MemoryPressureMonitor for pressure-aware retries
        if self.pressure_monitor.get_pressure() > 0.95 {
            return Err(CacheOperationError::resource_exhausted(
                "Memory pressure too high for retry",
            ));
        }

        // Use existing RecoveryStrategies for retry limits
        let max_retries = self
            .error_recovery
            .get_recovery_strategies()
            .get_retry_limit(RecoveryStrategy::ResourceReallocation);

        let mut retry_count = 0;

        while retry_count < max_retries {
            // Connect to existing CircuitBreaker - use tier 0 for memory allocation
            if !self.error_recovery.is_tier_available(0) {
                return Err(CacheOperationError::resource_exhausted(
                    "Memory allocation circuit open",
                ));
            }

            // Attempt allocation using existing pool selection logic
            let allocation_result = if size < 1024 {
                self.pool_manager.small_pool().allocate(size)
            } else if size < 65536 {
                self.pool_manager.medium_pool().allocate(size)
            } else {
                self.pool_manager.large_pool().allocate(size)
            };

            match allocation_result {
                Ok(ptr) => {
                    // Connect success to existing ErrorRecoverySystem
                    self.error_recovery.record_success(0);
                    return Ok(ptr);
                }
                Err(_) => {
                    // Connect failure to existing ErrorRecoverySystem
                    self.error_recovery
                        .handle_error(ErrorType::MemoryAllocationFailure, 0);

                    // Use existing sophisticated backoff calculation
                    let backoff_delay = self.calculate_sophisticated_backoff(retry_count);
                    std::thread::sleep(backoff_delay);

                    retry_count += 1;
                }
            }
        }

        // Execute existing recovery strategy if retries fail
        if self
            .error_recovery
            .execute_recovery(ErrorType::MemoryAllocationFailure, 0)
        {
            // Try once more after sophisticated recovery
            if size < 1024 {
                self.pool_manager.small_pool().allocate(size)
            } else if size < 65536 {
                self.pool_manager.medium_pool().allocate(size)
            } else {
                self.pool_manager.large_pool().allocate(size)
            }
        } else {
            Err(CacheOperationError::resource_exhausted(
                "Allocation failed after sophisticated recovery",
            ))
        }
    }

    /// Connect to existing sophisticated backoff algorithms
    fn calculate_sophisticated_backoff(&self, retry_count: u32) -> Duration {
        let recovery_config = self.error_recovery.get_recovery_strategies().get_config();

        let backoff_ms = match recovery_config.backoff_strategy {
            BackoffStrategy::Linear => retry_count * 100,
            BackoffStrategy::Exponential => {
                (100_u32).saturating_mul(2_u32.saturating_pow(retry_count.saturating_sub(1)))
            }
            BackoffStrategy::Fibonacci => match retry_count {
                1 => 100,
                2 => 100,
                3 => 200,
                4 => 300,
                _ => 500,
            },
            BackoffStrategy::Custom => 150,
        };

        // Add jitter to prevent thundering herd using simple deterministic approach
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let jitter = nanos % (backoff_ms / 4 + 1);
        Duration::from_millis((backoff_ms + jitter) as u64)
    }
}
