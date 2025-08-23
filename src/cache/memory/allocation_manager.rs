//! Core allocation manager with atomic statistics tracking
//!
//! This module implements the main memory allocation interface with comprehensive
//! statistics tracking and optimal pool selection for high-performance allocation.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossbeam_utils::CachePadded;

use std::sync::Arc;

use super::pool_manager::MemoryPoolManager;
use super::pressure_monitor::MemoryPressureMonitor;
use super::types::MemoryStatistics;
use crate::cache::config::CacheConfig;
use crate::cache::manager::error_recovery::core::ErrorRecoverySystem;
use crate::cache::manager::error_recovery::types::{BackoffStrategy, ErrorType, RecoveryStrategy};
use crate::cache::manager::background::types::{MaintenanceConfig, MaintenanceScheduler};
use crate::cache::memory::efficiency_analyzer::MemoryEfficiencyAnalyzer;
use crate::cache::memory::pool_manager::cleanup_manager::PoolCleanupManager;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Advanced memory manager with atomic allocation tracking
#[derive(Debug)]
pub struct AllocationManager {
    /// Global allocation statistics
    allocation_stats: Arc<AllocationStatistics>,
    /// Memory pool manager for efficient allocation
    pool_manager: MemoryPoolManager,
    /// Error recovery system for sophisticated retry mechanisms
    error_recovery: ErrorRecoverySystem,
    /// Memory pressure monitor for pressure-aware retries
    pressure_monitor: MemoryPressureMonitor,
}

/// Atomic allocation statistics for performance monitoring
#[derive(Debug)]
pub struct AllocationStatistics {
    /// Total allocated memory (bytes)
    total_allocated: CachePadded<AtomicU64>,
    /// Peak allocation reached (bytes)
    peak_allocation: CachePadded<AtomicU64>,
    /// Current active allocations count
    active_allocations: CachePadded<AtomicUsize>,
    /// Total allocation operations performed
    allocation_operations: CachePadded<AtomicU64>,
    /// Total deallocation operations performed
    deallocation_operations: CachePadded<AtomicU64>,
    /// Failed allocation attempts
    allocation_failures: CachePadded<AtomicU64>,
    /// Memory fragmentation level (percentage * 100)
    fragmentation_level: CachePadded<AtomicU32>,
    /// Average allocation size (bytes)
    avg_allocation_size: CachePadded<AtomicUsize>,
}

impl AllocationManager {
    /// Create new allocation manager with configuration
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        // Create shared allocation statistics for system integration
        let allocation_stats = Arc::new(AllocationStatistics::new());
        
        // Create sophisticated systems with shared references
        let efficiency_analyzer = Arc::new(MemoryEfficiencyAnalyzer::new(config)?);
        let maintenance_scheduler = Arc::new(MaintenanceScheduler::new(MaintenanceConfig::default())?);
        
        // Create cleanup manager that integrates existing systems
        let cleanup_manager = Arc::new(PoolCleanupManager::new(
            efficiency_analyzer,
            maintenance_scheduler,
            allocation_stats.clone(),
        ));
        
        // Create pool manager with cleanup integration
        let pool_manager = MemoryPoolManager::new_with_cleanup(config, Some(cleanup_manager))?;
        
        Ok(Self {
            allocation_stats,
            pool_manager,
            error_recovery: ErrorRecoverySystem::new(),
            pressure_monitor: MemoryPressureMonitor::new(config)?,
        })
    }

    /// Allocate memory with optimal pool selection
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        let _timer = Instant::now();

        // Update allocation statistics
        self.allocation_stats
            .total_allocated
            .fetch_add(size as u64, Ordering::Relaxed);
        self.allocation_stats
            .allocation_operations
            .fetch_add(1, Ordering::Relaxed);
        self.allocation_stats
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

                Ok(ptr)
            }
            Err(_) => {
                // Handle allocation failure
                self.allocation_stats
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
        self.allocation_stats
            .total_allocated
            .fetch_sub(size as u64, Ordering::Relaxed);
        self.allocation_stats
            .deallocation_operations
            .fetch_add(1, Ordering::Relaxed);
        self.allocation_stats
            .active_allocations
            .fetch_sub(1, Ordering::Relaxed);

        // Return memory to appropriate pool
        if size < 1024 {
            self.pool_manager.small_pool().deallocate(ptr, size)
        } else if size < 65536 {
            self.pool_manager.medium_pool().deallocate(ptr, size)
        } else {
            self.pool_manager.large_pool().deallocate(ptr, size)
        }
    }

    /// Get current memory statistics
    pub fn get_memory_stats(&self) -> MemoryStatistics {
        MemoryStatistics {
            total_allocated: self
                .allocation_stats
                .total_allocated
                .load(Ordering::Relaxed),
            peak_allocation: self
                .allocation_stats
                .peak_allocation
                .load(Ordering::Relaxed),
            active_allocations: self
                .allocation_stats
                .active_allocations
                .load(Ordering::Relaxed),
            allocation_operations: self
                .allocation_stats
                .allocation_operations
                .load(Ordering::Relaxed),
            deallocation_operations: self
                .allocation_stats
                .deallocation_operations
                .load(Ordering::Relaxed),
            allocation_failures: self
                .allocation_stats
                .allocation_failures
                .load(Ordering::Relaxed),
            fragmentation_level: self
                .allocation_stats
                .fragmentation_level
                .load(Ordering::Relaxed) as f32
                / 100.0,
            pressure_level: self.calculate_memory_pressure() as f32 / 100.0,
        }
    }

    /// Get allocation statistics reference
    pub fn allocation_stats(&self) -> &Arc<AllocationStatistics> {
        &self.allocation_stats
    }

    /// Get pool manager reference
    pub fn pool_manager(&self) -> &MemoryPoolManager {
        &self.pool_manager
    }

    /// Update peak allocation atomically
    fn update_peak_allocation(&self) {
        let current_total = self
            .allocation_stats
            .total_allocated
            .load(Ordering::Relaxed);
        let mut current_peak = self
            .allocation_stats
            .peak_allocation
            .load(Ordering::Relaxed);

        while current_total > current_peak {
            match self.allocation_stats.peak_allocation.compare_exchange_weak(
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
        let current_avg = self
            .allocation_stats
            .avg_allocation_size
            .load(Ordering::Relaxed);

        // Exponential moving average with alpha = 0.1
        let new_avg = (current_avg * 9 + size) / 10;
        self.allocation_stats
            .avg_allocation_size
            .store(new_avg, Ordering::Relaxed);
    }

    /// Handle allocation failure with recovery strategies
    fn handle_allocation_failure(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        // Record failure in existing atomic statistics (already exists)
        self.allocation_stats
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
        let total_allocated = self
            .allocation_stats
            .total_allocated
            .load(Ordering::Relaxed);
        let active_allocations = self
            .allocation_stats
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
        if self.pressure_monitor.current_pressure() > 0.95 {
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
                return Err(CacheOperationError::circuit_breaker_open(
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
            BackoffStrategy::Exponential => (100_u32)
                .saturating_mul(2_u32.saturating_pow(retry_count.saturating_sub(1))),
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
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
        let jitter = (nanos % (backoff_ms / 4 + 1)) as u32;
        Duration::from_millis((backoff_ms + jitter) as u64)
    }
}

impl AllocationStatistics {
    pub fn new() -> Self {
        Self {
            total_allocated: CachePadded::new(AtomicU64::new(0)),
            peak_allocation: CachePadded::new(AtomicU64::new(0)),
            active_allocations: CachePadded::new(AtomicUsize::new(0)),
            allocation_operations: CachePadded::new(AtomicU64::new(0)),
            deallocation_operations: CachePadded::new(AtomicU64::new(0)),
            allocation_failures: CachePadded::new(AtomicU64::new(0)),
            fragmentation_level: CachePadded::new(AtomicU32::new(0)),
            avg_allocation_size: CachePadded::new(AtomicUsize::new(1024)), // 1KB default
        }
    }
}

impl Clone for AllocationStatistics {
    fn clone(&self) -> Self {
        Self {
            total_allocated: CachePadded::new(AtomicU64::new(
                self.total_allocated.load(Ordering::Relaxed)
            )),
            peak_allocation: CachePadded::new(AtomicU64::new(
                self.peak_allocation.load(Ordering::Relaxed)
            )),
            active_allocations: CachePadded::new(AtomicUsize::new(
                self.active_allocations.load(Ordering::Relaxed)
            )),
            allocation_operations: CachePadded::new(AtomicU64::new(
                self.allocation_operations.load(Ordering::Relaxed)
            )),
            deallocation_operations: CachePadded::new(AtomicU64::new(
                self.deallocation_operations.load(Ordering::Relaxed)
            )),
            allocation_failures: CachePadded::new(AtomicU64::new(
                self.allocation_failures.load(Ordering::Relaxed)
            )),
            fragmentation_level: CachePadded::new(AtomicU32::new(
                self.fragmentation_level.load(Ordering::Relaxed)
            )),
            avg_allocation_size: CachePadded::new(AtomicUsize::new(
                self.avg_allocation_size.load(Ordering::Relaxed)
            )),
        }
    }
}

impl AllocationStatistics {
    /// Get total allocated memory
    pub fn total_allocated(&self) -> u64 {
        self.total_allocated.load(Ordering::Relaxed)
    }

    /// Get peak allocation
    pub fn peak_allocation(&self) -> u64 {
        self.peak_allocation.load(Ordering::Relaxed)
    }

    /// Get active allocations count
    pub fn active_allocations(&self) -> usize {
        self.active_allocations.load(Ordering::Relaxed)
    }

    /// Get allocation operations count
    pub fn allocation_operations(&self) -> u64 {
        self.allocation_operations.load(Ordering::Relaxed)
    }

    /// Get deallocation operations count
    pub fn deallocation_operations(&self) -> u64 {
        self.deallocation_operations.load(Ordering::Relaxed)
    }

    /// Get allocation failures count
    pub fn allocation_failures(&self) -> u64 {
        self.allocation_failures.load(Ordering::Relaxed)
    }

    /// Get fragmentation level
    pub fn fragmentation_level(&self) -> f32 {
        self.fragmentation_level.load(Ordering::Relaxed) as f32 / 100.0
    }

    /// Get average allocation size
    pub fn avg_allocation_size(&self) -> usize {
        self.avg_allocation_size.load(Ordering::Relaxed)
    }

    /// Update fragmentation level atomically (used by cleanup systems)
    pub fn update_fragmentation_level(&self, level: f32) {
        let level_as_u32 = (level * 100.0) as u32;
        self.fragmentation_level.store(level_as_u32, Ordering::Relaxed);
    }
}
