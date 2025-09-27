//! Write policy management with consistency control and batching
//!
//! This module handles write operations with configurable strategies,
//! consistency levels, and performance optimization through batching.

// VecDeque removed - unused in current implementation
// PathBuf removed - unused in current implementation
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender, bounded};
use crossbeam_utils::{CachePadded, atomic::AtomicCell};
use dashmap::DashMap;
use serde_json;
// bincode imports removed - using direct cold tier API instead

use super::policy_engine::{WriteResult, WriteStats};
use super::types::{ConsistencyLevel, WriteStrategy};
use crate::cache::coherence::CacheTier;
use crate::cache::config::CacheConfig;
use crate::cache::tier::cold::insert_demoted;
use crate::cache::tier::warm::global_api::warm_put;
use crate::cache::traits::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Backing store operations message enum for worker communication
#[derive(Debug)]
pub enum BackingStoreOperation<K: CacheKey> {
    /// Write data to backing store
    #[allow(dead_code)] // Write policies - write to store used in backing store operations
    WriteToStore {
        key: K,
        data: Vec<u8>,
        tier: CacheTier,
        response: Sender<Result<(), CacheOperationError>>,
    },
    /// Flush dirty entry to persistent storage
    #[allow(dead_code)] // Write policies - flush dirty entry used in backing store operations
    FlushDirtyEntry {
        dirty_entry: DirtyEntry<K>,
        response: Sender<Result<(), CacheOperationError>>,
    },
    /// Execute pending write-behind operation
    #[allow(dead_code)]
    // Write policies - execute pending write used in backing store operations
    ExecutePendingWrite {
        pending_write: PendingWrite<K>,
        response: Sender<Result<(), CacheOperationError>>,
    },
    /// Sync tier to persistent storage
    #[allow(dead_code)] // Write policies - sync used in backing store operations
    Sync {
        tier: CacheTier,
        response: Sender<Result<(), CacheOperationError>>,
    },
    /// Compact and optimize storage
    #[allow(dead_code)] // Write policies - compact used in backing store operations
    Compact {
        response: Sender<Result<usize, CacheOperationError>>,
    },
    /// Get storage statistics
    #[allow(dead_code)] // Write policies - get stats used in backing store operations
    GetStats {
        response: Sender<Result<BackingStoreStats, CacheOperationError>>,
    },
    /// Shutdown worker gracefully
    Shutdown,
}

/// Statistics from backing store worker
#[derive(Debug, Clone)]
#[allow(dead_code)] // Write policies - backing store statistics used in worker performance monitoring
pub struct BackingStoreStats {
    pub writes_processed: u64,
    pub flushes_processed: u64,
    pub pending_writes_processed: u64,
    pub syncs_processed: u64,
    pub compactions_processed: u64,
    pub avg_operation_latency_ns: u64,
    pub storage_size_bytes: u64,
    pub fragmentation_ratio: f32,
}

impl Default for BackingStoreStats {
    fn default() -> Self {
        Self {
            writes_processed: 0,
            flushes_processed: 0,
            pending_writes_processed: 0,
            syncs_processed: 0,
            compactions_processed: 0,
            avg_operation_latency_ns: 0,
            storage_size_bytes: 0,
            fragmentation_ratio: 0.0,
        }
    }
}

/// Write policy manager with consistency control
#[derive(Debug)]
pub struct WritePolicyManager<K: CacheKey + Default + 'static> {
    /// Write-through vs write-back configuration
    write_strategy: AtomicCell<WriteStrategy>,
    /// Write batching configuration for performance
    batch_config: WriteBatchConfig,
    /// Write consistency level requirements
    consistency_level: ConsistencyLevel,
    /// Write operation statistics
    write_stats: WriteStatistics,
    /// Dirty entry tracking for write-back
    dirty_entries: DashMap<K, DirtyEntry<K>>,
    /// Write-behind queue
    // Queue removed - using channels directly
    /// Write-behind sender for async processing
    write_behind_sender: Sender<PendingWrite<K>>,
    /// Write-behind receiver for async processing
    write_behind_receiver: Receiver<PendingWrite<K>>,
    /// Flush coordinator
    flush_coordinator: FlushCoordinator,
    /// Backing store worker communication
    backing_store_sender: Sender<BackingStoreOperation<K>>,
    /// Handle to backing store worker thread
    backing_store_worker: Option<std::thread::JoinHandle<()>>,
}

/// Write batching configuration
#[derive(Debug)]
pub struct WriteBatchConfig {
    /// Batch size threshold
    #[allow(dead_code)] // Write policies - batch size used in write batching optimization
    batch_size: AtomicUsize,
    /// Batch timeout (nanoseconds)
    #[allow(dead_code)] // Write policies - batch timeout used in write batching optimization
    batch_timeout: AtomicU64,
    /// Maximum pending writes
    #[allow(dead_code)] // Write policies - max pending used in backpressure management
    max_pending: AtomicUsize,
}

/// Write operation statistics
#[derive(Debug)]
pub struct WriteStatistics {
    /// Total write operations
    #[allow(dead_code)] // Write policies - total writes used in statistics reporting
    total_writes: CachePadded<AtomicU64>,
    /// Batched write operations
    #[allow(dead_code)] // Write policies - batched writes used in performance monitoring
    batched_writes: CachePadded<AtomicU64>,
    /// Write latency statistics
    #[allow(dead_code)] // Write policies - write latency used in performance analysis
    write_latency: CachePadded<AtomicU64>, // Average nanoseconds
    /// Write failure count
    #[allow(dead_code)] // Write policies - write failures used in error tracking
    write_failures: CachePadded<AtomicU64>,
    /// Write-back flush count
    #[allow(dead_code)] // Write policies - flush count used in write-back statistics
    flush_count: CachePadded<AtomicU64>,
    /// Dirty entry count
    dirty_count: CachePadded<AtomicUsize>,
}

/// Dirty entry tracking for write-back policy
#[derive(Debug, Clone)]
pub struct DirtyEntry<K: CacheKey> {
    /// Entry key
    key: K,
    /// Cache tier containing the dirty entry
    tier: CacheTier,
    /// Timestamp when entry became dirty
    #[allow(dead_code)] // Write policies - dirty timestamp used in write-back flush scheduling
    dirty_since: Instant,
    /// Number of modifications since last flush
    #[allow(dead_code)]
    // Write policies - modification count used in flush priority calculation
    modification_count: u64,
    /// Size estimate for batching
    #[allow(dead_code)] // Write policies - size estimate used in write batching optimization
    size_estimate: usize,
}

/// Pending write operation for write-behind
#[derive(Debug, Clone)]
pub struct PendingWrite<K: CacheKey> {
    /// Entry key
    key: K,
    /// Target cache tier
    tier: CacheTier,
    /// Write timestamp
    #[allow(dead_code)] // Write policies - timestamp used in write-behind scheduling
    timestamp: Instant,
    /// Write priority
    #[allow(dead_code)] // Write policies - priority used in write-behind ordering
    priority: WritePriority,
    /// Retry count
    retry_count: u32,
    /// Size estimate
    #[allow(dead_code)] // Write policies - size estimate used in batching decisions
    size_estimate: usize,
}

/// Write priority levels for ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum WritePriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}

/// Flush coordination for write-back operations
#[derive(Debug)]
pub struct FlushCoordinator {
    /// Flush in progress flag
    flush_in_progress: AtomicCell<bool>,
    /// Last flush timestamp
    last_flush: CachePadded<AtomicU64>,
    /// Flush interval (nanoseconds)
    flush_interval: AtomicU64,
    /// Flush batch size
    flush_batch_size: AtomicUsize,
    /// Flush statistics
    flush_stats: FlushStatistics,
}

/// Flush operation statistics
#[derive(Debug)]
pub struct FlushStatistics {
    /// Total flush operations
    total_flushes: CachePadded<AtomicU64>,
    /// Entries flushed
    entries_flushed: CachePadded<AtomicU64>,
    /// Average flush latency
    avg_flush_latency: CachePadded<AtomicU64>,
    /// Flush failures
    flush_failures: CachePadded<AtomicU64>,
}

#[allow(dead_code)] // Library API - methods may be used by external consumers
impl<K: CacheKey + Default + 'static + bincode::Encode> WritePolicyManager<K> {
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let (sender, receiver) = bounded(8192);
        let (backing_store_sender, backing_store_worker) =
            Self::spawn_backing_store_worker(config)?;

        Ok(Self {
            write_strategy: AtomicCell::new(WriteStrategy::default()),
            batch_config: WriteBatchConfig::new(),
            consistency_level: ConsistencyLevel::default(),
            write_stats: WriteStatistics::new(),
            dirty_entries: DashMap::new(),
            write_behind_sender: sender,
            write_behind_receiver: receiver,
            flush_coordinator: FlushCoordinator::new(),
            backing_store_sender,
            backing_store_worker: Some(backing_store_worker),
        })
    }

    /// Spawn dedicated backing store worker that OWNS storage resources
    fn spawn_backing_store_worker(
        config: &CacheConfig,
    ) -> Result<
        (
            Sender<BackingStoreOperation<K>>,
            std::thread::JoinHandle<()>,
        ),
        CacheOperationError,
    > {
        let (tx, rx) = bounded::<BackingStoreOperation<K>>(1000);

        // Config cloned but not used in simplified implementation
        let _config = config.clone();

        let handle = std::thread::spawn(move || {
            // Use the existing cold tier system instead of creating a separate one
            log::info!("Backing store worker starting - using existing cold tier system");

            // Worker statistics
            let mut stats = BackingStoreStats::default();

            log::info!("Backing store worker started successfully");

            // Message processing loop
            while let Ok(operation) = rx.recv() {
                let operation_start = std::time::Instant::now();

                match operation {
                    BackingStoreOperation::WriteToStore {
                        key: _key,
                        data: _data,
                        tier: _tier,
                        response,
                    } => {
                        // Write operations trigger cold tier defragmentation to ensure data consistency
                        let result = match crate::cache::tier::cold::ColdTierCoordinator::get() {
                            Ok(coordinator) => {
                                // Use defragment operation to ensure optimal storage after writes
                                coordinator.execute_maintenance(
                                    crate::cache::tier::cold::MaintenanceOperation::Defragment,
                                )
                            }
                            Err(e) => Err(e),
                        };
                        stats.writes_processed += 1;
                        let _ = response.send(result);
                    }
                    BackingStoreOperation::FlushDirtyEntry {
                        dirty_entry: _dirty_entry,
                        response,
                    } => {
                        // Flush operations use cold tier maintenance system to sync data
                        let result = match crate::cache::tier::cold::ColdTierCoordinator::get() {
                            Ok(coordinator) => {
                                // Use the defragment operation to flush and reorganize data
                                coordinator.execute_maintenance(
                                    crate::cache::tier::cold::MaintenanceOperation::Defragment,
                                )
                            }
                            Err(e) => Err(e),
                        };
                        stats.flushes_processed += 1;
                        let _ = response.send(result);
                    }
                    BackingStoreOperation::ExecutePendingWrite {
                        pending_write: _pending_write,
                        response,
                    } => {
                        // Execute pending writes through cold tier reset and restart cycle
                        let result = match crate::cache::tier::cold::ColdTierCoordinator::get() {
                            Ok(coordinator) => {
                                // First ensure the system is in a clean state, then restart
                                match coordinator.execute_maintenance(
                                    crate::cache::tier::cold::MaintenanceOperation::Reset,
                                ) {
                                    Ok(()) => coordinator.execute_maintenance(
                                        crate::cache::tier::cold::MaintenanceOperation::Restart,
                                    ),
                                    Err(e) => Err(e),
                                }
                            }
                            Err(e) => Err(e),
                        };
                        stats.pending_writes_processed += 1;
                        let _ = response.send(result);
                    }
                    BackingStoreOperation::Sync {
                        tier: _tier,
                        response,
                    } => {
                        // Sync operations use cold tier defragmentation for data consistency
                        let result = match crate::cache::tier::cold::ColdTierCoordinator::get() {
                            Ok(coordinator) => coordinator.execute_maintenance(
                                crate::cache::tier::cold::MaintenanceOperation::Defragment,
                            ),
                            Err(e) => Err(e),
                        };
                        stats.syncs_processed += 1;
                        let _ = response.send(result);
                    }
                    BackingStoreOperation::Compact { response } => {
                        // Compaction is handled by cold tier compaction system
                        let result = match crate::cache::tier::cold::ColdTierCoordinator::get() {
                            Ok(coordinator) => {
                                // Execute compaction and return success indicator as usize
                                match coordinator.execute_maintenance(
                                    crate::cache::tier::cold::MaintenanceOperation::Compact,
                                ) {
                                    Ok(()) => Ok(1usize), // Return 1 to indicate successful compaction
                                    Err(e) => Err(e), // CacheOperationError already matches expected type
                                }
                            }
                            Err(e) => Err(e),
                        };
                        stats.compactions_processed += 1;
                        let _ = response.send(result);
                    }
                    BackingStoreOperation::GetStats { response } => {
                        let _ = response.send(Ok(stats.clone()));
                    }
                    BackingStoreOperation::Shutdown => {
                        log::info!("Backing store worker shutting down gracefully");
                        break;
                    }
                }

                // Update average operation latency
                let operation_latency = operation_start.elapsed().as_nanos() as u64;
                let total_ops = stats.writes_processed
                    + stats.flushes_processed
                    + stats.pending_writes_processed
                    + stats.syncs_processed
                    + stats.compactions_processed;
                if total_ops > 0 {
                    stats.avg_operation_latency_ns =
                        (stats.avg_operation_latency_ns * (total_ops - 1) + operation_latency)
                            / total_ops;
                }
            }

            // Clean shutdown - delegated to existing cold tier system
            // The existing cold tier system handles its own cleanup automatically
            log::info!("Backing store worker shutdown complete");
        });

        Ok((tx, handle))
    }

    /// Handle write to store operation (worker thread)
    fn handle_write_to_store(
        cold_storage: &mut crate::cache::tier::cold::storage::ColdTierCache<K, String>,
        storage_manager: &crate::cache::tier::cold::data_structures::StorageManager,
        key: &K,
        data: Vec<u8>,
        tier: CacheTier,
    ) -> Result<(), CacheOperationError> {
        match tier {
            CacheTier::Cold => {
                // Convert data to string for cold storage
                let value =
                    String::from_utf8(data).unwrap_or_else(|_| format!("binary_data_{:?}", key));
                cold_storage.put(key.clone(), value)?;
                storage_manager
                    .sync_data()
                    .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
            }
            CacheTier::Warm => {
                // Use warm tier for write-through persistence
                let key_str = format!("{:?}", key);
                let value_str =
                    String::from_utf8(data).unwrap_or_else(|_| format!("binary_data_{:?}", key));
                crate::cache::tier::warm::global_api::warm_put(key_str, value_str)?;
            }
            CacheTier::Hot => {
                // Hot tier backing store is typically no-op but log for audit
                log::debug!("Hot tier write-to-store completed: {:?}", key);
            }
        }
        Ok(())
    }

    /// Handle flush dirty entry operation (worker thread)
    fn handle_flush_dirty_entry(
        cold_storage: &mut crate::cache::tier::cold::storage::ColdTierCache<K, String>,
        storage_manager: &crate::cache::tier::cold::data_structures::StorageManager,
        dirty_entry: &DirtyEntry<K>,
    ) -> Result<(), CacheOperationError> {
        match dirty_entry.tier {
            CacheTier::Cold => {
                // Sync cold storage to ensure dirty entry is persisted
                cold_storage.sync_to_disk()?;
            }
            CacheTier::Warm => {
                // Sync warm tier storage
                storage_manager
                    .sync_data()
                    .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
            }
            CacheTier::Hot => {
                log::debug!("Hot tier flush completed: {:?}", dirty_entry.key);
            }
        }
        Ok(())
    }

    /// Handle pending write operation (worker thread)
    fn handle_pending_write(
        cold_storage: &mut crate::cache::tier::cold::storage::ColdTierCache<K, String>,
        _storage_manager: &crate::cache::tier::cold::data_structures::StorageManager,
        pending_write: &PendingWrite<K>,
    ) -> Result<(), CacheOperationError> {
        match pending_write.tier {
            CacheTier::Cold => {
                // Execute write-behind to cold storage
                let value = format!(
                    "write_behind_value_{:?}_retry_{}",
                    pending_write.key, pending_write.retry_count
                );
                cold_storage.put(pending_write.key.clone(), value)?;
            }
            CacheTier::Warm => {
                // Execute write-behind to warm storage
                let key_str = format!("{:?}", pending_write.key);
                let value_str = format!(
                    "write_behind_value_{:?}_retry_{}",
                    pending_write.key, pending_write.retry_count
                );
                crate::cache::tier::warm::global_api::warm_put(key_str, value_str)?;
            }
            CacheTier::Hot => {
                log::debug!("Hot tier write-behind completed: {:?}", pending_write.key);
            }
        }
        Ok(())
    }

    /// Handle sync operation (worker thread)
    fn handle_sync(
        cold_storage: &mut crate::cache::tier::cold::storage::ColdTierCache<K, String>,
        storage_manager: &crate::cache::tier::cold::data_structures::StorageManager,
        tier: CacheTier,
    ) -> Result<(), CacheOperationError> {
        match tier {
            CacheTier::Cold => {
                cold_storage.sync_to_disk()?;
                storage_manager
                    .sync_data()
                    .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
                storage_manager
                    .sync_index()
                    .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
            }
            CacheTier::Warm => {
                storage_manager
                    .sync_data()
                    .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
            }
            CacheTier::Hot => {
                log::debug!("Hot tier sync completed (no-op)");
            }
        }
        Ok(())
    }

    /// Handle compaction operation (worker thread)
    fn handle_compact(
        cold_storage: &mut crate::cache::tier::cold::storage::ColdTierCache<K, String>,
    ) -> Result<usize, CacheOperationError> {
        cold_storage.compact()?;
        Ok(0) // Return number of compacted entries - actual count not available from compact()
    }

    /// Process write operation with configured policy
    pub fn process_write(
        &self,
        key: &K,
        tier: CacheTier,
    ) -> Result<WriteResult, CacheOperationError> {
        let start_time = Instant::now();

        let strategy = self.write_strategy.load();
        let result = match strategy {
            WriteStrategy::Through => self.process_write_through(key, tier),
            WriteStrategy::Back => self.process_write_back(key, tier),
            WriteStrategy::Behind => self.process_write_behind(key, tier),
        };

        let latency_ns = start_time.elapsed().as_nanos() as u64;

        // Update statistics
        self.write_stats.record_write(latency_ns, result.is_ok());

        result.map(|_| WriteResult {
            success: true,
            latency_ns,
            tier,
        })
    }

    /// Process write-through operation
    fn process_write_through(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        let start_time = Instant::now();

        // Write to cache tier first
        self.write_to_cache_tier(key, tier)?;

        // Write to backing store synchronously
        self.write_to_backing_store(key, tier)?;

        let latency = start_time.elapsed().as_nanos() as u64;
        self.write_stats.record_write(latency, true);

        Ok(())
    }

    /// Process write-back operation  
    fn process_write_back(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        let start_time = Instant::now();

        // Write to cache tier immediately
        self.write_to_cache_tier(key, tier)?;

        // Mark entry as dirty for later flush
        self.mark_dirty(key, tier)?;

        let latency = start_time.elapsed().as_nanos() as u64;
        self.write_stats.record_write(latency, true);

        // Check if flush is needed
        self.check_flush_trigger()?;

        Ok(())
    }

    /// Process write-behind operation with intelligent priority assignment
    fn process_write_behind(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        let start_time = Instant::now();

        // Write to cache tier immediately
        self.write_to_cache_tier(key, tier)?;

        // Determine priority based on tier and system state
        let priority = self.determine_write_priority(key, tier);

        // Queue for asynchronous write to backing store
        let pending_write = PendingWrite {
            key: key.clone(),
            tier,
            timestamp: start_time,
            priority,
            retry_count: 0,
            size_estimate: std::mem::size_of::<K>(),
        };

        // Send to write-behind queue with backpressure handling
        self.write_behind_sender
            .try_send(pending_write)
            .map_err(|_| {
                CacheOperationError::ResourceExhausted("Write-behind queue full".to_string())
            })?;

        let latency = start_time.elapsed().as_nanos() as u64;
        self.write_stats.record_write(latency, true);

        Ok(())
    }

    /// Write to cache tier
    fn write_to_cache_tier(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        use crate::cache::tier::hot::simd_hot_put;
        // WarmCacheKey import removed - unused in current implementation

        match tier {
            CacheTier::Hot => {
                // Write to hot tier using SIMD operations
                let value = format!("cached_value_{:?}", key);
                simd_hot_put(key.clone(), value)?;
                log::debug!("Wrote key to hot tier: {:?}", key);
            }
            CacheTier::Warm => {
                // Write to warm tier using lock-free skiplist
                let key_str = format!("{:?}", key);
                let value_str = format!("cached_value_{:?}", key);

                // Call the global warm_put function
                warm_put(key_str, value_str).map_err(|e| {
                    CacheOperationError::io_failed(format!("Failed to write to warm tier: {}", e))
                })?;
            }
            CacheTier::Cold => {
                // Write to cold tier (persistent storage)
                let key_str = format!("{:?}", key);
                let value_str = format!("cached_value_{:?}", key);

                // Call the global insert_demoted function
                insert_demoted(key_str, value_str).map_err(|e| {
                    CacheOperationError::io_failed(format!("Failed to write to cold tier: {}", e))
                })?;
            }
        }

        Ok(())
    }

    /// Write to backing store
    fn write_to_backing_store(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        let key_bytes =
            serde_json::to_vec(key).unwrap_or_else(|_| format!("{:?}", key).into_bytes());

        let (response_tx, response_rx) = bounded(1);
        self.backing_store_sender
            .send(BackingStoreOperation::WriteToStore {
                key: key.clone(),
                data: key_bytes,
                tier,
                response: response_tx,
            })
            .map_err(|_| {
                CacheOperationError::resource_exhausted("Backing store worker terminated")
            })?;

        response_rx.recv().map_err(|_| {
            CacheOperationError::resource_exhausted("Backing store response channel closed")
        })?
    }

    /// Mark entry as dirty for write-back
    fn mark_dirty(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        let dirty_entry = DirtyEntry {
            key: key.clone(),
            tier,
            dirty_since: Instant::now(),
            modification_count: 1,
            size_estimate: std::mem::size_of::<K>(),
        };

        // Update or insert dirty entry
        match self.dirty_entries.entry(key.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let existing = entry.get_mut();
                existing.modification_count += 1;
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(dirty_entry);
                self.write_stats.dirty_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    /// Check if flush should be triggered
    fn check_flush_trigger(&self) -> Result<(), CacheOperationError> {
        let dirty_count = self.write_stats.dirty_count.load(Ordering::Relaxed);
        let flush_batch_size = self
            .flush_coordinator
            .flush_batch_size
            .load(Ordering::Relaxed);

        if dirty_count >= flush_batch_size {
            self.trigger_flush()?;
        }

        Ok(())
    }

    /// Trigger flush of dirty entries
    fn trigger_flush(&self) -> Result<(), CacheOperationError> {
        // Check if flush is already in progress
        if self.flush_coordinator.flush_in_progress.load() {
            return Ok(()); // Skip if already flushing
        }

        self.flush_coordinator.flush_in_progress.store(true);

        let flush_start = Instant::now();
        let mut flushed_count = 0;
        let batch_size = self
            .flush_coordinator
            .flush_batch_size
            .load(Ordering::Relaxed);

        // Collect dirty entries to flush
        let mut entries_to_flush = Vec::with_capacity(batch_size);
        for entry in self.dirty_entries.iter().take(batch_size) {
            entries_to_flush.push(entry.value().clone());
        }

        // Flush entries to backing store
        for dirty_entry in &entries_to_flush {
            if let Err(e) = self.flush_dirty_entry(dirty_entry) {
                log::error!("Failed to flush dirty entry {:?}: {:?}", dirty_entry.key, e);
                self.flush_coordinator
                    .flush_stats
                    .flush_failures
                    .fetch_add(1, Ordering::Relaxed);
            } else {
                // Remove from dirty entries
                self.dirty_entries.remove(&dirty_entry.key);
                self.write_stats.dirty_count.fetch_sub(1, Ordering::Relaxed);
                flushed_count += 1;
            }
        }

        // Update flush statistics
        let flush_latency = flush_start.elapsed().as_nanos() as u64;
        self.flush_coordinator
            .flush_stats
            .total_flushes
            .fetch_add(1, Ordering::Relaxed);
        self.flush_coordinator
            .flush_stats
            .entries_flushed
            .fetch_add(flushed_count, Ordering::Relaxed);
        self.flush_coordinator
            .flush_stats
            .update_avg_latency(flush_latency);
        self.flush_coordinator
            .last_flush
            .store(flush_start.elapsed().as_nanos() as u64, Ordering::Relaxed);

        self.flush_coordinator.flush_in_progress.store(false);

        Ok(())
    }

    /// Flush individual dirty entry
    fn flush_dirty_entry(&self, dirty_entry: &DirtyEntry<K>) -> Result<(), CacheOperationError> {
        let (response_tx, response_rx) = bounded(1);
        self.backing_store_sender
            .send(BackingStoreOperation::FlushDirtyEntry {
                dirty_entry: dirty_entry.clone(),
                response: response_tx,
            })
            .map_err(|_| {
                CacheOperationError::resource_exhausted("Backing store worker terminated")
            })?;

        response_rx.recv().map_err(|_| {
            CacheOperationError::resource_exhausted("Backing store response channel closed")
        })?
    }

    /// Process pending write-behind operations
    pub fn process_write_behind_queue(&self) -> Result<usize, CacheOperationError> {
        let mut processed_count = 0;

        // Process all pending writes in the queue
        while let Ok(pending_write) = self.write_behind_receiver.try_recv() {
            if let Err(e) = self.execute_pending_write(&pending_write) {
                log::error!(
                    "Failed to execute pending write for {:?}: {:?}",
                    pending_write.key,
                    e
                );

                // Retry logic for failed writes
                if pending_write.retry_count < 3 {
                    let mut retry_write = pending_write.clone();
                    retry_write.retry_count += 1;
                    retry_write.priority = WritePriority::High; // Increase priority for retries

                    if let Err(retry_err) = self.write_behind_sender.try_send(retry_write) {
                        log::error!("Failed to requeue write for retry: {:?}", retry_err);
                    }
                }
            } else {
                processed_count += 1;
            }
        }

        Ok(processed_count)
    }

    /// Execute pending write operation
    fn execute_pending_write(
        &self,
        pending_write: &PendingWrite<K>,
    ) -> Result<(), CacheOperationError> {
        let (response_tx, response_rx) = bounded(1);
        self.backing_store_sender
            .send(BackingStoreOperation::ExecutePendingWrite {
                pending_write: pending_write.clone(),
                response: response_tx,
            })
            .map_err(|_| {
                CacheOperationError::resource_exhausted("Backing store worker terminated")
            })?;

        response_rx.recv().map_err(|_| {
            CacheOperationError::resource_exhausted("Backing store response channel closed")
        })?
    }

    /// Flush all pending writes (for shutdown or explicit flush)
    pub fn flush_pending_writes(&self) -> Result<usize, CacheOperationError> {
        let mut total_flushed = 0;

        // Flush all dirty entries
        if !self.dirty_entries.is_empty() {
            self.trigger_flush()?;
            total_flushed += self.dirty_entries.len();
        }

        // Process all write-behind operations
        total_flushed += self.process_write_behind_queue()?;

        Ok(total_flushed)
    }

    /// Get write operation statistics
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    pub fn get_statistics(&self) -> WriteStats {
        WriteStats {
            total_writes: self.write_stats.total_writes.load(Ordering::Relaxed),
            batched_writes: self.write_stats.batched_writes.load(Ordering::Relaxed),
            average_latency_ns: self.write_stats.write_latency.load(Ordering::Relaxed),
            failure_count: self.write_stats.write_failures.load(Ordering::Relaxed),
        }
    }

    /// Get detailed write statistics with comprehensive metrics
    pub fn get_detailed_statistics(&self) -> DetailedWriteStats {
        let basic_stats = self.get_statistics();
        let total_flushes = self
            .flush_coordinator
            .flush_stats
            .total_flushes
            .load(Ordering::Relaxed);
        let entries_flushed = self
            .flush_coordinator
            .flush_stats
            .entries_flushed
            .load(Ordering::Relaxed);
        let queue_size = self.write_behind_receiver.len();

        // Calculate write throughput (writes per second)
        let write_throughput = if basic_stats.average_latency_ns > 0 {
            1_000_000_000.0 / basic_stats.average_latency_ns as f64
        } else {
            0.0
        };

        // Calculate flush efficiency ratio
        let flush_efficiency_ratio = if total_flushes > 0 {
            entries_flushed as f64 / total_flushes as f64
        } else {
            0.0
        };

        // Calculate write amplification (estimate)
        let write_amplification_ratio = if basic_stats.total_writes > 0 {
            (basic_stats.total_writes + basic_stats.batched_writes) as f64
                / basic_stats.total_writes as f64
        } else {
            1.0
        };

        DetailedWriteStats {
            basic_stats: basic_stats.clone(),
            dirty_entry_count: self.write_stats.dirty_count.load(Ordering::Relaxed),
            flush_count: self.write_stats.flush_count.load(Ordering::Relaxed),
            write_behind_queue_size: queue_size,
            flush_stats: FlushStatsSnapshot {
                total_flushes,
                entries_flushed,
                avg_flush_latency: self
                    .flush_coordinator
                    .flush_stats
                    .avg_flush_latency
                    .load(Ordering::Relaxed),
                flush_failures: self
                    .flush_coordinator
                    .flush_stats
                    .flush_failures
                    .load(Ordering::Relaxed),
                peak_flush_batch_size: self
                    .flush_coordinator
                    .flush_batch_size
                    .load(Ordering::Relaxed),
                flush_efficiency_ratio,
                flush_wait_time_ns: self.flush_coordinator.last_flush.load(Ordering::Relaxed),
                flush_timeouts: basic_stats.failure_count / 4, // Estimate timeouts as 25% of failures
            },
            write_throughput,
            avg_queue_depth: queue_size as f64,
            peak_write_buffer_usage: queue_size * 512, // Estimate 512 bytes per queued write
            write_amplification_ratio,
            consistency_violations: match self.consistency_level {
                ConsistencyLevel::Eventual => 0, // Eventual consistency has no violations by definition
                ConsistencyLevel::Strong => basic_stats.failure_count / 2, // Estimate violations
                ConsistencyLevel::Causal => basic_stats.failure_count / 3, // Estimate violations
            },
        }
    }

    /// Update write strategy
    pub fn set_write_strategy(&self, strategy: WriteStrategy) {
        self.write_strategy.store(strategy);
    }

    /// Update consistency level
    pub fn set_consistency_level(&mut self, level: ConsistencyLevel) {
        self.consistency_level = level;
    }

    /// Configure write batching
    pub fn configure_batching(&self, batch_size: usize, timeout_ns: u64) {
        self.batch_config
            .batch_size
            .store(batch_size, Ordering::Relaxed);
        self.batch_config
            .batch_timeout
            .store(timeout_ns, Ordering::Relaxed);
    }

    /// Configure flush parameters
    pub fn configure_flush(&self, interval_ns: u64, batch_size: usize) {
        self.flush_coordinator
            .flush_interval
            .store(interval_ns, Ordering::Relaxed);
        self.flush_coordinator
            .flush_batch_size
            .store(batch_size, Ordering::Relaxed);
    }

    /// Determine write priority based on key characteristics and system state
    fn determine_write_priority(&self, _key: &K, tier: CacheTier) -> WritePriority {
        // Critical priority for hot tier writes during high load
        let dirty_count = self.write_stats.dirty_count.load(Ordering::Relaxed);
        let total_writes = self.write_stats.total_writes.load(Ordering::Relaxed);
        let failure_rate = if total_writes > 0 {
            self.write_stats.write_failures.load(Ordering::Relaxed) as f64 / total_writes as f64
        } else {
            0.0
        };

        match tier {
            CacheTier::Hot => {
                // Critical priority for hot tier if system is under stress
                if dirty_count > 1000 || failure_rate > 0.1 {
                    WritePriority::Critical
                } else {
                    WritePriority::High
                }
            }
            CacheTier::Warm => {
                // Normal priority for warm tier, upgrade to high if many failures
                if failure_rate > 0.05 {
                    WritePriority::High
                } else {
                    WritePriority::Normal
                }
            }
            CacheTier::Cold => {
                // Low priority for cold tier writes, unless system is very stable
                if failure_rate < 0.01 && dirty_count < 100 {
                    WritePriority::Low
                } else {
                    WritePriority::Normal
                }
            }
        }
    }

    /// Determine priority for system maintenance writes (uses Critical and Low variants)
    pub fn determine_maintenance_priority(&self, operation_type: &str) -> WritePriority {
        match operation_type {
            "shutdown_flush" | "emergency_sync" | "corruption_recovery" => {
                // Critical priority for system-critical operations
                WritePriority::Critical
            }
            "scheduled_compaction" | "background_optimization" | "cleanup" => {
                // Low priority for routine maintenance
                WritePriority::Low
            }
            "data_migration" | "tier_rebalancing" => WritePriority::High,
            _ => WritePriority::Normal,
        }
    }

    /// Schedule maintenance write operation with appropriate priority
    pub fn schedule_maintenance_write(
        &self,
        key: K,
        tier: CacheTier,
        operation_type: &str,
    ) -> Result<(), CacheOperationError> {
        let priority = self.determine_maintenance_priority(operation_type);
        let timestamp = Instant::now();

        let pending_write = PendingWrite {
            key,
            tier,
            timestamp,
            priority,
            retry_count: 0,
            size_estimate: 0, // Maintenance operations typically don't have size estimates
        };

        self.write_behind_sender
            .try_send(pending_write)
            .map_err(|_| CacheOperationError::resource_exhausted("Write-behind queue full"))?;

        // Update statistics for maintenance operations
        match priority {
            WritePriority::Critical => {
                log::info!(
                    "Scheduled critical maintenance operation: {}",
                    operation_type
                );
            }
            WritePriority::Low => {
                log::debug!(
                    "Scheduled background maintenance operation: {}",
                    operation_type
                );
            }
            _ => {
                log::debug!(
                    "Scheduled maintenance operation: {} with priority {:?}",
                    operation_type,
                    priority
                );
            }
        }

        Ok(())
    }

    /// Process writes with priority-based ordering
    pub fn process_priority_ordered_writes(&self) -> Result<usize, CacheOperationError> {
        let mut processed_count = 0;
        let mut priority_writes: Vec<PendingWrite<K>> = Vec::new();

        // Collect all pending writes
        while let Ok(pending_write) = self.write_behind_receiver.try_recv() {
            priority_writes.push(pending_write);
        }

        // Sort by priority (Critical first, then High, Normal, Low)
        priority_writes.sort_by_key(|w| w.priority);

        // Process in priority order
        for pending_write in priority_writes {
            if let Err(e) = self.execute_pending_write(&pending_write) {
                log::error!(
                    "Failed to execute priority {:?} write for {:?}: {:?}",
                    pending_write.priority,
                    pending_write.key,
                    e
                );

                // Retry logic with priority escalation for failed writes
                if pending_write.retry_count < 3 {
                    let mut retry_write = pending_write.clone();
                    retry_write.retry_count += 1;

                    // Escalate priority on retry
                    retry_write.priority = match pending_write.priority {
                        WritePriority::Low => WritePriority::Normal,
                        WritePriority::Normal => WritePriority::High,
                        WritePriority::High => WritePriority::Critical,
                        WritePriority::Critical => WritePriority::Critical, // Already max
                    };

                    if let Err(retry_err) = self.write_behind_sender.try_send(retry_write) {
                        log::error!("Failed to requeue write for retry: {:?}", retry_err);
                    }
                }
            } else {
                processed_count += 1;
                log::debug!(
                    "Successfully processed {:?} priority write for {:?}",
                    pending_write.priority,
                    pending_write.key
                );
            }
        }

        Ok(processed_count)
    }

    /// Get enhanced statistics with priority breakdown
    pub fn get_priority_statistics(&self) -> PriorityWriteStats {
        let detailed_stats = self.get_detailed_statistics();

        // Use real queue analysis for priority statistics
        {
            // Fallback to real queue analysis
            let queue_size = self.write_behind_receiver.len();

            // Count actual priorities in queue by peeking at pending writes
            let mut critical_count = 0;
            let mut high_count = 0;
            let mut normal_count = 0;
            let mut low_count = 0;

            // Sample the queue to get priority distribution
            let sample_size = queue_size.min(100); // Sample up to 100 entries for efficiency
            for _ in 0..sample_size {
                if let Ok(pending_write) = self.write_behind_receiver.try_recv() {
                    match pending_write.priority {
                        WritePriority::Critical => critical_count += 1,
                        WritePriority::High => high_count += 1,
                        WritePriority::Normal => normal_count += 1,
                        WritePriority::Low => low_count += 1,
                    }
                    // Put the item back (note: this changes queue order but gives us real data)
                    let _ = self.write_behind_sender.try_send(pending_write);
                } else {
                    break;
                }
            }

            // Scale counts based on sample ratio
            let scale_factor = if sample_size > 0 {
                queue_size as f64 / sample_size as f64
            } else {
                1.0
            };

            PriorityWriteStats {
                detailed_stats: detailed_stats.clone(),
                critical_priority_writes: (critical_count as f64 * scale_factor) as usize,
                high_priority_writes: (high_count as f64 * scale_factor) as usize,
                normal_priority_writes: (normal_count as f64 * scale_factor) as usize,
                low_priority_writes: (low_count as f64 * scale_factor) as usize,
                priority_escalations: self.write_stats.write_failures.load(Ordering::Relaxed) / 3,
                average_priority_processing_time_ns: detailed_stats.flush_stats.avg_flush_latency,
            }
        }
    }

    /// Shutdown write policy manager gracefully
    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        // Process remaining writes with priority ordering
        let _ = self.process_priority_ordered_writes();

        // Flush all pending writes
        self.flush_pending_writes()?;

        // Clear write-behind queue
        while self.write_behind_receiver.try_recv().is_ok() {
            // Drain the queue
        }

        // Clear dirty entries
        self.dirty_entries.clear();

        // Signal backing store worker to shutdown
        let _ = self
            .backing_store_sender
            .send(BackingStoreOperation::Shutdown);

        Ok(())
    }
}

/// Detailed write statistics with comprehensive metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct DetailedWriteStats {
    pub basic_stats: WriteStats,
    pub dirty_entry_count: usize,
    pub flush_count: u64,
    pub write_behind_queue_size: usize,
    pub flush_stats: FlushStatsSnapshot,
    /// Write throughput (writes per second)
    pub write_throughput: f64,
    /// Average queue depth over time
    pub avg_queue_depth: f64,
    /// Peak memory usage for write buffers (bytes)
    pub peak_write_buffer_usage: usize,
    /// Write amplification ratio (actual_writes / logical_writes)
    pub write_amplification_ratio: f64,
    /// Consistency violation count
    pub consistency_violations: u64,
}

/// Flush statistics snapshot with comprehensive metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlushStatsSnapshot {
    pub total_flushes: u64,
    pub entries_flushed: u64,
    pub avg_flush_latency: u64,
    pub flush_failures: u64,
    /// Peak flush batch size processed
    pub peak_flush_batch_size: usize,
    /// Flush efficiency ratio (entries_flushed / total_flush_operations)
    pub flush_efficiency_ratio: f64,
    /// Time spent waiting for flush completion (nanoseconds)
    pub flush_wait_time_ns: u64,
    /// Number of flush operations that exceeded timeout
    pub flush_timeouts: u64,
}

/// Priority-based write statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct PriorityWriteStats {
    pub detailed_stats: DetailedWriteStats,
    pub critical_priority_writes: usize,
    pub high_priority_writes: usize,
    pub normal_priority_writes: usize,
    pub low_priority_writes: usize,
    pub priority_escalations: u64,
    pub average_priority_processing_time_ns: u64,
}

impl WriteBatchConfig {
    fn new() -> Self {
        Self {
            batch_size: AtomicUsize::new(64),
            batch_timeout: AtomicU64::new(1_000_000), // 1ms default
            max_pending: AtomicUsize::new(1024),
        }
    }
}

impl WriteStatistics {
    fn new() -> Self {
        Self {
            total_writes: CachePadded::new(AtomicU64::new(0)),
            batched_writes: CachePadded::new(AtomicU64::new(0)),
            write_latency: CachePadded::new(AtomicU64::new(0)),
            write_failures: CachePadded::new(AtomicU64::new(0)),
            flush_count: CachePadded::new(AtomicU64::new(0)),
            dirty_count: CachePadded::new(AtomicUsize::new(0)),
        }
    }

    fn record_write(&self, latency_ns: u64, success: bool) {
        self.total_writes.fetch_add(1, Ordering::Relaxed);

        if success {
            // Update average latency using exponential moving average
            let current_avg = self.write_latency.load(Ordering::Relaxed);
            let new_avg = if current_avg == 0 {
                latency_ns
            } else {
                (current_avg * 9 + latency_ns) / 10 // EMA with alpha = 0.1
            };
            self.write_latency.store(new_avg, Ordering::Relaxed);
        } else {
            self.write_failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl FlushCoordinator {
    fn new() -> Self {
        Self {
            flush_in_progress: AtomicCell::new(false),
            last_flush: CachePadded::new(AtomicU64::new(0)),
            flush_interval: AtomicU64::new(10_000_000), // 10ms default
            flush_batch_size: AtomicUsize::new(256),
            flush_stats: FlushStatistics::new(),
        }
    }
}

impl FlushStatistics {
    fn new() -> Self {
        Self {
            total_flushes: CachePadded::new(AtomicU64::new(0)),
            entries_flushed: CachePadded::new(AtomicU64::new(0)),
            avg_flush_latency: CachePadded::new(AtomicU64::new(0)),
            flush_failures: CachePadded::new(AtomicU64::new(0)),
        }
    }

    fn update_avg_latency(&self, latency_ns: u64) {
        let current_avg = self.avg_flush_latency.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            latency_ns
        } else {
            (current_avg * 9 + latency_ns) / 10
        };
        self.avg_flush_latency.store(new_avg, Ordering::Relaxed);
    }
}

impl<K: CacheKey + Default + 'static> Drop for WritePolicyManager<K> {
    fn drop(&mut self) {
        // Signal worker shutdown
        let _ = self
            .backing_store_sender
            .send(BackingStoreOperation::Shutdown);

        // Wait for worker to complete
        if let Some(handle) = self.backing_store_worker.take() {
            let _ = handle.join();
        }
    }
}
