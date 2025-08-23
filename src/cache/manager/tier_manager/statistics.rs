//! Statistics and queue management for tier promotion system
//!
//! This module implements lock-free statistics tracking and priority queue
//! management for promotion/demotion operations.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crossbeam_skiplist::SkipMap;

use super::types::{
    PromotionPriority, PromotionQueue, PromotionStatistics, PromotionTask, TierLocation,
};

impl PromotionStatistics {
    /// Create new promotion statistics
    #[inline]
    pub fn new() -> Self {
        Self {
            hot_to_warm_promotions: AtomicU64::new(0),
            warm_to_hot_promotions: AtomicU64::new(0),
            cold_to_warm_promotions: AtomicU64::new(0),
            warm_to_cold_demotions: AtomicU64::new(0),
            avg_decision_time: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
            success_rate_x1000: AtomicU32::new(0),
            simd_operations_count: AtomicU64::new(0),
        }
    }

    /// Record promotion operation
    #[inline(always)]
    pub fn record_promotion(&self, from: TierLocation, to: TierLocation, decision_time_ns: u64) {
        match (from, to) {
            (TierLocation::Warm, TierLocation::Hot) => {
                self.warm_to_hot_promotions.fetch_add(1, Ordering::Relaxed);
            }
            (TierLocation::Cold, TierLocation::Warm) => {
                self.cold_to_warm_promotions.fetch_add(1, Ordering::Relaxed);
            }
            (TierLocation::Hot, TierLocation::Warm) => {
                self.hot_to_warm_promotions.fetch_add(1, Ordering::Relaxed);
            }
            (TierLocation::Warm, TierLocation::Cold) => {
                self.warm_to_cold_demotions.fetch_add(1, Ordering::Relaxed);
            }
            _ => {} // Invalid transitions
        }

        // Update average decision time
        let current_avg = self.avg_decision_time.load(Ordering::Relaxed);
        let total_ops = self.total_operations.fetch_add(1, Ordering::Relaxed);
        let new_avg = (current_avg * total_ops + decision_time_ns) / (total_ops + 1);
        self.avg_decision_time.store(new_avg, Ordering::Relaxed);
    }
}

impl<K: crate::cache::traits::CacheKey> PromotionQueue<K> {
    /// Create new promotion queue
    pub fn new(max_size: u32) -> Self {
        Self {
            task_queue: SkipMap::new(),
            queue_size: AtomicU32::new(0),
            max_queue_size: AtomicU32::new(max_size),
            tasks_processed: AtomicU64::new(0),
            tasks_dropped: AtomicU64::new(0),
            total_processing_time: AtomicU64::new(0),
        }
    }

    /// Submit task to promotion queue
    #[inline]
    pub fn submit_task(&self, task: PromotionTask<K>) -> Result<(), PromotionTask<K>> {
        let current_size = self.queue_size.load(Ordering::Relaxed);
        let max_size = self.max_queue_size.load(Ordering::Relaxed);

        if current_size >= max_size {
            self.tasks_dropped.fetch_add(1, Ordering::Relaxed);
            return Err(task);
        }

        let priority = PromotionPriority {
            urgency: task.priority_score as u16,
            timestamp_ns: task.created_at.elapsed().as_nanos() as u64,
            sequence: current_size,
        };

        self.task_queue.insert(priority, task);
        self.queue_size.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get next task from queue
    #[inline]
    pub fn get_next_task(&self) -> Option<PromotionTask<K>> {
        if let Some(entry) = self.task_queue.pop_front() {
            self.queue_size.fetch_sub(1, Ordering::Relaxed);
            self.tasks_processed.fetch_add(1, Ordering::Relaxed);
            Some(entry.value().clone())
        } else {
            None
        }
    }

    /// Get queue statistics
    #[inline]
    pub fn get_stats(&self) -> (u32, u64, u64, u64) {
        (
            self.queue_size.load(Ordering::Relaxed),
            self.tasks_processed.load(Ordering::Relaxed),
            self.tasks_dropped.load(Ordering::Relaxed),
            self.total_processing_time.load(Ordering::Relaxed),
        )
    }

    /// Update queue processing time
    #[inline]
    pub fn record_processing_time(&self, processing_time_ns: u64) {
        self.total_processing_time
            .fetch_add(processing_time_ns, Ordering::Relaxed);
    }
}
