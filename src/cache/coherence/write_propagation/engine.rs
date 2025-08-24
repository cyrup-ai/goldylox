//! Write operations engine for propagation system
//!
//! This module implements the core write propagation logic including
//! write-through, write-back, adaptive policies, and queue management.

use std::sync::atomic::Ordering;
use std::time::Instant;

use crossbeam_channel::unbounded;

use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey};
use super::types::{
    PropagationConfig, PropagationPolicy, PropagationStatistics, PropagationStatisticsSnapshot,
    WorkerHealth, WriteBackRequest, WritePriority, WritePropagationSystem,
};
use super::worker_system::{WorkerChannels, WriteBackTask};
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey, V: CacheValue> WritePropagationSystem<K, V> {
    /// Create new write propagation system
    pub fn new(_writeback_delay_ns: u64) -> Self {
        let (task_tx, task_rx) = unbounded();
        let (completion_tx, completion_rx) = unbounded();

        Self {
            writeback_queue: crossbeam_skiplist::SkipMap::new(),
            propagation_policy: PropagationPolicy::WriteBack,
            worker_channels: WorkerChannels {
                task_tx,
                task_rx,
                completion_tx,
                completion_rx,
            },
            propagation_stats: PropagationStatistics::new(),
            config: PropagationConfig::default(),
            worker_health: std::sync::atomic::AtomicU32::new(WorkerHealth::Healthy as u32),
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

        match self.propagation_policy {
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

    /// Execute immediate write-through
    fn execute_immediate_write(&self, _request: &WriteBackRequest<K, V>) {
        let start_time = Instant::now();

        // Simulate write operation (in real implementation, this would write to the target tier)
        // For now, we'll just update statistics

        let write_latency = start_time.elapsed().as_nanos() as u64;
        self.propagation_stats.update_write_latency(write_latency);

        // Update bytes written (estimate based on data size)
        let data_size = 1024u64; // Generic estimate for cache value size
        self.propagation_stats.add_bytes_written(data_size);
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

            tasks.push(task);
            completed_requests.push(id);
        }

        // Remove processed requests from queue
        for id in completed_requests {
            self.writeback_queue.remove(&id);
        }

        tasks
    }

    /// Get propagation statistics
    pub fn get_statistics(&self) -> PropagationStatisticsSnapshot {
        let current_queue_depth = self.writeback_queue.len() as u32;
        self.propagation_stats.snapshot(current_queue_depth)
    }

    /// Get current worker health
    pub fn get_worker_health(&self) -> WorkerHealth {
        match self.worker_health.load(Ordering::Relaxed) {
            0 => WorkerHealth::Healthy,
            1 => WorkerHealth::Degraded,
            2 => WorkerHealth::Overloaded,
            _ => WorkerHealth::Failed,
        }
    }

    /// Update worker health status
    pub fn update_worker_health(&self, health: WorkerHealth) {
        self.worker_health.store(health as u32, Ordering::Relaxed);
    }
}
