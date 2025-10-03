//! Write operations engine for propagation system
//!
//! This module implements the core write propagation logic including
//! write-through, write-back, adaptive policies, and queue management.

use std::sync::atomic::Ordering;
use std::time::Instant;

use crossbeam_channel::unbounded;

use super::types::{
    PropagationConfig, PropagationPolicy, PropagationStatistics, WorkerHealth, WriteBackRequest,
    WritePriority, WritePropagationSystem,
};
use super::worker_system::{WorkerChannels, WriteBackTask};
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey};
use crate::cache::traits::{CacheKey, CacheValue};

impl<
    K: CacheKey
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
    V: CacheValue
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
> WritePropagationSystem<K, V>
{
    /// Create new write propagation system
    pub fn new(
        writeback_delay_ns: u64,
        hot_tier_coordinator: std::sync::Arc<crate::cache::tier::hot::thread_local::HotTierCoordinator>,
        warm_tier_coordinator: std::sync::Arc<crate::cache::tier::warm::global_api::WarmTierCoordinator>,
        cold_tier_coordinator: std::sync::Arc<crate::cache::tier::cold::ColdTierCoordinator>,
    ) -> Self {
        let (task_tx, task_rx) = unbounded();
        let (completion_tx, completion_rx) = unbounded();

        // Select configuration preset based on delay requirements
        let config = if writeback_delay_ns < 50_000_000 {
            // < 50ms
            PropagationConfig::low_latency()
        } else if writeback_delay_ns > 200_000_000 {
            // > 200ms
            PropagationConfig::memory_constrained()
        } else {
            PropagationConfig::high_throughput()
        };

        // Validate configuration
        if let Err(e) = config.validate() {
            log::warn!("Invalid write propagation config: {}", e);
        }

        let initial_policy = if writeback_delay_ns < 100_000_000 {
            // < 100ms
            PropagationPolicy::WriteThrough // Low latency requirement
        } else {
            PropagationPolicy::WriteBack // Normal operation
        };

        Self {
            writeback_queue: crossbeam_skiplist::SkipMap::new(),
            propagation_policy: std::sync::atomic::AtomicU8::new(initial_policy as u8),
            worker_channels: WorkerChannels {
                task_tx,
                task_rx,
                completion_tx,
                completion_rx,
            },
            propagation_stats: PropagationStatistics::new(),
            config,
            worker_health: std::sync::atomic::AtomicU32::new(WorkerHealth::Healthy as u32),
            hot_tier_coordinator,
            warm_tier_coordinator,
            cold_tier_coordinator,
        }
    }

    /// Submit write-back request
    pub fn submit_writeback(
        &self,
        key: CoherenceKey<K>,
        data: V,
        source_tier: CacheTier,
        target_tier: CacheTier,
        version: u64,
        priority: WritePriority,
    ) -> u64 {
        let request_id = self
            .propagation_stats
            .writebacks
            .fetch_add(1, Ordering::AcqRel);

        let request = WriteBackRequest {
            key,
            data,
            source_tier,
            target_tier,
            version,
            created_at: Instant::now(),
            priority,
        };

        // Update priority statistics
        self.update_priority_stats(priority);

        let current_policy = match self
            .propagation_policy
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            0 => PropagationPolicy::WriteThrough,
            1 => PropagationPolicy::WriteBack,
            2 => PropagationPolicy::Adaptive,
            _ => PropagationPolicy::WriteBack, // Default fallback
        };

        match current_policy {
            PropagationPolicy::WriteThrough => {
                // Immediate write-through
                self.execute_immediate_write(&request);
                self.propagation_stats
                    .propagations
                    .fetch_add(1, Ordering::Relaxed);
            }
            PropagationPolicy::WriteBack => {
                // Queue for background processing
                self.writeback_queue.insert(request_id, request);
                self.update_queue_depth();
            }
            PropagationPolicy::Adaptive => {
                // Decide based on current system state
                if self.should_write_through(&request) {
                    self.execute_immediate_write(&request);
                    self.propagation_stats
                        .propagations
                        .fetch_add(1, Ordering::Relaxed);
                } else {
                    self.writeback_queue.insert(request_id, request);
                    self.update_queue_depth();
                }
            }
        }

        request_id
    }

    /// Update priority-specific statistics
    fn update_priority_stats(&self, priority: WritePriority) {
        match priority {
            WritePriority::Low => self
                .propagation_stats
                .low_priority_writes
                .fetch_add(1, Ordering::Relaxed),
            WritePriority::Normal => self
                .propagation_stats
                .normal_priority_writes
                .fetch_add(1, Ordering::Relaxed),
            WritePriority::High => self
                .propagation_stats
                .high_priority_writes
                .fetch_add(1, Ordering::Relaxed),
            WritePriority::Critical => self
                .propagation_stats
                .critical_priority_writes
                .fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Execute immediate write-through to actual tier storage
    fn execute_immediate_write(&self, request: &WriteBackRequest<K, V>)
    where
        K: Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    {
        let start_time = Instant::now();

        // Extract the actual cache key from the coherence key
        let cache_key = request.key.original_key.clone();
        let cache_value = request.data.clone();

        // Perform actual write operation to target tier using crossbeam channels
        let write_result = match request.target_tier {
            CacheTier::Hot => {
                // Write to hot tier using SIMD-optimized crossbeam messaging
                crate::cache::tier::hot::thread_local::simd_hot_put(&self.hot_tier_coordinator, cache_key, cache_value)
            }
            CacheTier::Warm => {
                // Write to warm tier using balanced crossbeam messaging
                crate::cache::tier::warm::global_api::warm_put(&self.warm_tier_coordinator, cache_key, cache_value)
            }
            CacheTier::Cold => {
                // Write to cold tier using insert_demoted function
                crate::cache::tier::cold::insert_demoted(&self.cold_tier_coordinator, cache_key, cache_value)
            }
        };

        // Record write completion and update statistics
        let write_latency = start_time.elapsed().as_nanos() as u64;
        self.propagation_stats.update_write_latency(write_latency);

        match write_result {
            Ok(_) => {
                // Successfully written - estimate data size for statistics
                let data_size = request.key.original_key.estimated_size() as u64
                    + std::mem::size_of::<V>() as u64; // Conservative estimate
                self.propagation_stats.add_bytes_written(data_size);

                log::debug!(
                    "Write-through completed successfully to {:?} tier",
                    request.target_tier
                );
            }
            Err(e) => {
                // Write failed - record error
                self.propagation_stats.record_failed_write();
                log::error!(
                    "Write-through failed to {:?} tier: {}",
                    request.target_tier,
                    e
                );
            }
        }
    }

    /// Determine if write should be immediate (for adaptive policy)
    fn should_write_through(&self, request: &WriteBackRequest<K, V>) -> bool {
        match request.priority {
            WritePriority::Critical => true,
            WritePriority::High => {
                // Write through if queue is getting full
                self.writeback_queue.len() > (self.config.max_queue_size as usize / 2)
            }
            _ => false,
        }
    }

    /// Update queue depth statistics
    fn update_queue_depth(&self) {
        let current_depth = self.writeback_queue.len() as u32;
        self.propagation_stats
            .update_peak_queue_depth(current_depth);
    }

    /// Process background write-back queue
    pub fn process_writebacks(&self) -> Vec<WriteBackTask<K, V>> {
        let mut tasks = Vec::new();
        let now = Instant::now();
        let mut completed_requests = Vec::new();

        // Collect requests that are ready for processing
        let mut requests: Vec<_> = self
            .writeback_queue
            .iter()
            .filter_map(|entry| {
                let (id, request) = (entry.key(), entry.value());
                let age_ns = now.duration_since(request.created_at).as_nanos() as u64;

                if age_ns >= self.config.writeback_delay_ns
                    || request.priority >= WritePriority::High
                {
                    Some((*id, request.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Sort by priority (high to low) then by age (old to new)
        requests.sort_by(|(_, a), (_, b)| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });

        // Create tasks up to batch size
        for (id, request) in requests.into_iter().take(self.config.batch_size as usize) {
            // Convert generic request to simplified format for compilation
            let simplified_request = WriteBackRequest {
                key: CoherenceKey::from_cache_key(&request.key),
                data: request.data.clone(),
                source_tier: request.source_tier,
                target_tier: request.target_tier,
                version: request.version,
                created_at: request.created_at,
                priority: request.priority,
            };

            let task = WriteBackTask::<K, V> {
                task_id: id,
                request: simplified_request,
                submitted_at: now,
                timeout_ns: self.config.writeback_delay_ns * 10, // 10x delay as timeout
            };

            // Submit task to worker via crossbeam channel
            if self.worker_channels.task_tx.try_send(task.clone()).is_err() {
                // Channel full - record worker task error
                self.propagation_stats.record_worker_error();
            } else {
                // Successfully queued - record worker task
                self.propagation_stats.record_worker_task();
            }

            tasks.push(task);
            completed_requests.push(id);
        }

        // Remove processed requests from queue
        for id in completed_requests {
            self.writeback_queue.remove(&id);
        }

        tasks
    }

    /// Process worker completion messages
    pub fn process_worker_completions(&self) {
        // Process all pending completion messages
        while let Ok(completion) = self.worker_channels.completion_rx.try_recv() {
            match completion.result {
                super::worker_system::WriteBackResult::Success => {
                    // Successfully completed - update statistics
                    self.propagation_stats.record_worker_task();
                    self.propagation_stats
                        .propagations
                        .fetch_add(1, Ordering::Relaxed);
                }
                super::worker_system::WriteBackResult::RetryableError { reason } => {
                    // Retryable error - log reason and record statistics
                    log::warn!("Write propagation retryable error: {}", reason);
                    self.propagation_stats.record_worker_task();
                    self.propagation_stats.record_failed_write();
                    self.propagation_stats.record_worker_error();
                }
                super::worker_system::WriteBackResult::PermanentError { reason } => {
                    // Permanent error - log reason and record statistics
                    log::error!("Write propagation permanent error: {}", reason);
                    self.propagation_stats.record_worker_task();
                    self.propagation_stats.record_failed_write();
                    self.propagation_stats.record_worker_error();
                }
                super::worker_system::WriteBackResult::Timeout => {
                    // Timeout - record statistics
                    log::warn!("Write propagation task timed out");
                    self.propagation_stats.record_worker_task();
                    self.propagation_stats.record_failed_write();
                    self.propagation_stats.record_worker_error();
                }
            }

            // Update write latency statistics
            self.propagation_stats
                .update_write_latency(completion.processing_time_ns);
        }

        // Adaptive policy switching based on system performance
        self.adapt_propagation_policy();
    }

    /// Adapt propagation policy based on current system performance
    fn adapt_propagation_policy(&self) {
        let queue_depth = self.writeback_queue.len() as u32;
        let failed_writes = self.propagation_stats.failed_writes.load(Ordering::Relaxed);
        let total_writes = self.propagation_stats.writebacks.load(Ordering::Relaxed);

        // Calculate failure rate
        let failure_rate = if total_writes > 0 {
            (failed_writes as f64 / total_writes as f64) * 100.0
        } else {
            0.0
        };

        // Switch to adaptive policy under certain conditions
        let should_use_adaptive =
            queue_depth > (self.config.max_queue_size / 2) || failure_rate > 5.0;

        if should_use_adaptive {
            // Update to adaptive policy for better performance under load
            self.propagation_policy.store(
                PropagationPolicy::Adaptive as u8,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    }

    /// Get current worker health
    #[allow(dead_code)] // Background workers - used in async task processing and worker coordination
    pub fn get_worker_health(&self) -> WorkerHealth {
        match self.worker_health.load(Ordering::Relaxed) {
            0 => WorkerHealth::Healthy,
            1 => WorkerHealth::Degraded,
            2 => WorkerHealth::Overloaded,
            _ => WorkerHealth::Failed,
        }
    }

    /// Update worker health status
    #[allow(dead_code)] // Background workers - used in async task processing and worker coordination
    pub fn update_worker_health(&self, health: WorkerHealth) {
        self.worker_health.store(health as u32, Ordering::Relaxed);
    }
}
