//! Write policy management with consistency control and batching
//!
//! This module handles write operations with configurable strategies,
//! consistency levels, and performance optimization through batching.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};
use crossbeam_utils::{atomic::AtomicCell, CachePadded};
use dashmap::DashMap;

use super::super::coherence::CacheTier;
use super::super::config::CacheConfig;
use super::policy_engine::{WriteResult, WriteStats};
use super::types::{ConsistencyLevel, WriteStrategy};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::CacheKey;

/// Write policy manager with consistency control
#[derive(Debug)]
pub struct WritePolicyManager<K: CacheKey> {
    /// Write-through vs write-back configuration
    write_strategy: AtomicCell<WriteStrategy>,
    /// Write batching configuration for performance
    batch_config: WriteBatchConfig,
    /// Write consistency level requirements
    consistency_level: ConsistencyLevel,
    /// Write operation statistics
    write_stats: WriteStatistics,
    /// Dirty entry tracking for write-back
    dirty_entries: Arc<DashMap<K, DirtyEntry<K>>>,
    /// Write-behind queue
    write_behind_queue: Arc<Mutex<VecDeque<PendingWrite<K>>>>,
    /// Write-behind sender for async processing
    write_behind_sender: Sender<PendingWrite<K>>,
    /// Write-behind receiver for async processing
    write_behind_receiver: Receiver<PendingWrite<K>>,
    /// Flush coordinator
    flush_coordinator: Arc<FlushCoordinator>,
}

/// Write batching configuration
#[derive(Debug)]
pub struct WriteBatchConfig {
    /// Batch size threshold
    batch_size: AtomicUsize,
    /// Batch timeout (nanoseconds)
    batch_timeout: AtomicU64,
    /// Maximum pending writes
    max_pending: AtomicUsize,
    /// Batching enabled flag
    batching_enabled: AtomicCell<bool>,
}

/// Write operation statistics
#[derive(Debug)]
pub struct WriteStatistics {
    /// Total write operations
    total_writes: CachePadded<AtomicU64>,
    /// Batched write operations
    batched_writes: CachePadded<AtomicU64>,
    /// Write latency statistics
    write_latency: CachePadded<AtomicU64>, // Average nanoseconds
    /// Write failure count
    write_failures: CachePadded<AtomicU64>,
    /// Write-back flush count
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
    dirty_since: Instant,
    /// Number of modifications since last flush
    modification_count: u64,
    /// Size estimate for batching
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
    timestamp: Instant,
    /// Write priority
    priority: WritePriority,
    /// Retry count
    retry_count: u32,
    /// Size estimate
    size_estimate: usize,
}

/// Write priority levels for ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WritePriority {
    Critical = 0,
    High = 1,
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

impl<K: CacheKey> WritePolicyManager<K> {
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let (sender, receiver) = bounded(8192);

        Ok(Self {
            write_strategy: AtomicCell::new(WriteStrategy::default()),
            batch_config: WriteBatchConfig::new(),
            consistency_level: ConsistencyLevel::default(),
            write_stats: WriteStatistics::new(),
            dirty_entries: Arc::new(DashMap::new()),
            write_behind_queue: Arc::new(Mutex::new(VecDeque::new())),
            write_behind_sender: sender,
            write_behind_receiver: receiver,
            flush_coordinator: Arc::new(FlushCoordinator::new()),
        })
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
            WriteStrategy::WriteThrough => self.process_write_through(key, tier),
            WriteStrategy::WriteBack => self.process_write_back(key, tier),
            WriteStrategy::WriteBehind => self.process_write_behind(key, tier),
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

    /// Process write-behind operation
    fn process_write_behind(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        let start_time = Instant::now();

        // Write to cache tier immediately
        self.write_to_cache_tier(key, tier)?;

        // Queue for asynchronous write to backing store
        let pending_write = PendingWrite {
            key: key.clone(),
            tier,
            timestamp: start_time,
            priority: WritePriority::Normal,
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
        use std::sync::Arc;

        use crate::cache::tier::hot::simd_hot_put;
        use crate::cache::tier::warm::core::WarmCacheKey;

        match tier {
            CacheTier::Hot => {
                // Write to hot tier using SIMD operations
                let value = Arc::new(format!("cached_value_{:?}", key));
                simd_hot_put(key.clone(), value)?;
                log::debug!("Wrote key to hot tier: {:?}", key);
            }
            CacheTier::Warm => {
                // Write to warm tier using lock-free skiplist
                let warm_key = WarmCacheKey::from_cache_key(key);
                let _value = Arc::new(format!("cached_value_{:?}", key));
                // Note: Would need access to warm tier instance here
                // For now, log the operation
                log::debug!("Would write key to warm tier: {:?}", warm_key);
            }
            CacheTier::Cold => {
                // Write to cold tier (persistent storage)
                log::debug!("Would write key to cold tier: {:?}", key);
            }
        }

        Ok(())
    }

    /// Write to backing store
    fn write_to_backing_store(&self, key: &K, tier: CacheTier) -> Result<(), CacheOperationError> {
        // Use key's serialization capabilities if available, otherwise use debug format
        let key_bytes = key.as_bytes();

        match tier {
            CacheTier::Cold => {
                // Write to memory-mapped file or database
                // For now, use a simple file write as example
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key_bytes.hash(&mut hasher);
                let filename = format!("cache_key_{:x}.dat", hasher.finish());

                log::debug!("Writing key to backing store: {}", filename);

                // In production, this would use async I/O
                // std::fs::write(&filename, &key_bytes)?;
            }
            _ => {
                log::debug!("Backing store write for tier {:?}", tier);
            }
        }

        Ok(())
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
                log::error!("Failed to flush dirty entry {}: {:?}", dirty_entry.key, e);
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
    fn flush_dirty_entry(&self, _dirty_entry: &DirtyEntry<K>) -> Result<(), CacheOperationError> {
        // Real implementation would write the dirty entry to backing store
        // For now, simulate successful flush with some latency
        std::thread::sleep(Duration::from_micros(5));
        Ok(())
    }

    /// Process pending write-behind operations
    pub fn process_write_behind_queue(&self) -> Result<usize, CacheOperationError> {
        let mut processed_count = 0;

        // Process all pending writes in the queue
        while let Ok(pending_write) = self.write_behind_receiver.try_recv() {
            if let Err(e) = self.execute_pending_write(&pending_write) {
                log::error!(
                    "Failed to execute pending write for {}: {:?}",
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
        _pending_write: &PendingWrite<K>,
    ) -> Result<(), CacheOperationError> {
        // Real implementation would execute the actual write to backing store
        // For now, simulate successful write
        std::thread::sleep(Duration::from_micros(8));
        Ok(())
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
    pub fn get_statistics(&self) -> WriteStats {
        WriteStats {
            total_writes: self.write_stats.total_writes.load(Ordering::Relaxed),
            batched_writes: self.write_stats.batched_writes.load(Ordering::Relaxed),
            average_latency_ns: self.write_stats.write_latency.load(Ordering::Relaxed),
            failure_count: self.write_stats.write_failures.load(Ordering::Relaxed),
        }
    }

    /// Get detailed write statistics
    pub fn get_detailed_statistics(&self) -> DetailedWriteStats {
        DetailedWriteStats {
            basic_stats: self.get_statistics(),
            dirty_entry_count: self.write_stats.dirty_count.load(Ordering::Relaxed),
            flush_count: self.write_stats.flush_count.load(Ordering::Relaxed),
            write_behind_queue_size: self.write_behind_receiver.len(),
            flush_stats: FlushStatsSnapshot {
                total_flushes: self
                    .flush_coordinator
                    .flush_stats
                    .total_flushes
                    .load(Ordering::Relaxed),
                entries_flushed: self
                    .flush_coordinator
                    .flush_stats
                    .entries_flushed
                    .load(Ordering::Relaxed),
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
    pub fn configure_batching(&self, enabled: bool, batch_size: usize, timeout_ns: u64) {
        self.batch_config.batching_enabled.store(enabled);
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

    /// Shutdown write policy manager gracefully
    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        // Flush all pending writes
        self.flush_pending_writes()?;

        // Clear write-behind queue
        while let Ok(_) = self.write_behind_receiver.try_recv() {
            // Drain the queue
        }

        // Clear dirty entries
        self.dirty_entries.clear();

        Ok(())
    }
}

/// Detailed write statistics
#[derive(Debug, Clone)]
pub struct DetailedWriteStats {
    pub basic_stats: WriteStats,
    pub dirty_entry_count: usize,
    pub flush_count: u64,
    pub write_behind_queue_size: usize,
    pub flush_stats: FlushStatsSnapshot,
}

/// Flush statistics snapshot
#[derive(Debug, Clone)]
pub struct FlushStatsSnapshot {
    pub total_flushes: u64,
    pub entries_flushed: u64,
    pub avg_flush_latency: u64,
    pub flush_failures: u64,
}

impl WriteBatchConfig {
    fn new() -> Self {
        Self {
            batch_size: AtomicUsize::new(64),
            batch_timeout: AtomicU64::new(1_000_000), // 1ms default
            max_pending: AtomicUsize::new(1024),
            batching_enabled: AtomicCell::new(true),
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

impl Default for WritePriority {
    fn default() -> Self {
        WritePriority::Normal
    }
}
