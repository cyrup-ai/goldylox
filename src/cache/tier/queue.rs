//! Lock-free promotion queue with atomic coordination and work-stealing support
//!
//! This module provides efficient task scheduling and processing for tier promotion
//! operations using lock-free data structures and priority-based ordering.

#![allow(dead_code)] // Tier management - Complete lock-free promotion queue library with work-stealing support

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender, unbounded};
use crossbeam_skiplist::SkipMap;
use crossbeam_utils::CachePadded;

use crate::cache::coherence::CacheTier;
use crate::cache::traits::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Lock-free promotion queue with atomic coordination
#[derive(Debug)]
pub struct PromotionQueue<K: CacheKey> {
    /// High-priority promotion tasks (lock-free skiplist for O(log n) operations)
    high_priority: SkipMap<PromotionPriority, PromotionTask<K>>,
    /// Normal-priority promotion tasks
    normal_priority: SkipMap<PromotionPriority, PromotionTask<K>>,
    /// Low-priority promotion tasks (background optimization)
    low_priority: SkipMap<PromotionPriority, PromotionTask<K>>,
    /// Atomic queue size counters for load balancing
    queue_sizes: CachePadded<[AtomicUsize; 3]>, // High, Normal, Low
    /// Processing channels for work-stealing scheduler
    task_sender: Sender<PromotionTask<K>>,
    task_receiver: Receiver<PromotionTask<K>>,
}

/// Promotion task with atomic scheduling information
#[derive(Debug, Clone)]
pub struct PromotionTask<K: CacheKey> {
    /// Cache key to promote
    pub key: K,
    /// Current tier location
    pub from_tier: CacheTier,
    /// Target tier for promotion
    pub to_tier: CacheTier,
    /// Task priority for scheduling
    pub priority: PromotionPriority,
    /// Task creation timestamp
    pub created_at: Instant,
    /// Expected completion time
    pub deadline: Instant,
}

/// Promotion priority with atomic ordering for skiplist
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PromotionPriority {
    /// Priority level (higher = more important)
    pub level: u8,
    /// Creation timestamp for FIFO within same priority
    pub timestamp: u64, // Nanoseconds since epoch
}

impl<K: CacheKey> PromotionQueue<K> {
    pub fn new() -> Result<Self, CacheOperationError> {
        let (task_sender, task_receiver) = unbounded();

        Ok(Self {
            high_priority: SkipMap::new(),
            normal_priority: SkipMap::new(),
            low_priority: SkipMap::new(),
            queue_sizes: CachePadded::new([
                AtomicUsize::new(0),
                AtomicUsize::new(0),
                AtomicUsize::new(0),
            ]),
            task_sender,
            task_receiver,
        })
    }

    /// Schedule a promotion task in the appropriate priority queue
    pub fn schedule_task(&self, task: PromotionTask<K>) -> Result<(), CacheOperationError> {
        // Add to appropriate priority queue
        match task.priority.level {
            8..=10 => {
                self.high_priority.insert(task.priority, task.clone());
                self.queue_sizes[0].fetch_add(1, Ordering::Relaxed);
            }
            4..=7 => {
                self.normal_priority.insert(task.priority, task.clone());
                self.queue_sizes[1].fetch_add(1, Ordering::Relaxed);
            }
            0..=3 => {
                self.low_priority.insert(task.priority, task.clone());
                self.queue_sizes[2].fetch_add(1, Ordering::Relaxed);
            }
            _ => {
                return Err(CacheOperationError::invalid_argument(
                    "Invalid priority level",
                ));
            }
        }

        // Send to work-stealing scheduler
        self.task_sender
            .send(task)
            .map_err(|_| CacheOperationError::resource_exhausted("Promotion queue full"))?;

        Ok(())
    }

    /// Get next task from queue (work-stealing compatible)
    pub fn get_next_task(&self) -> Result<Option<PromotionTask<K>>, CacheOperationError> {
        // Try receiving from work-stealing channel first
        if let Ok(task) = self.task_receiver.try_recv() {
            return Ok(Some(task));
        }

        // Try getting from priority queues in order
        if let Some(entry) = self.high_priority.pop_front() {
            self.queue_sizes[0].fetch_sub(1, Ordering::Relaxed);
            return Ok(Some(entry.value().clone()));
        }

        if let Some(entry) = self.normal_priority.pop_front() {
            self.queue_sizes[1].fetch_sub(1, Ordering::Relaxed);
            return Ok(Some(entry.value().clone()));
        }

        if let Some(entry) = self.low_priority.pop_front() {
            self.queue_sizes[2].fetch_sub(1, Ordering::Relaxed);
            return Ok(Some(entry.value().clone()));
        }

        Ok(None)
    }

    /// Get current queue sizes for monitoring
    pub fn get_queue_sizes(&self) -> (usize, usize, usize) {
        (
            self.queue_sizes[0].load(Ordering::Relaxed),
            self.queue_sizes[1].load(Ordering::Relaxed),
            self.queue_sizes[2].load(Ordering::Relaxed),
        )
    }

    /// Get total number of pending tasks
    pub fn total_pending_tasks(&self) -> usize {
        let (high, normal, low) = self.get_queue_sizes();
        high + normal + low
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.total_pending_tasks() == 0
    }

    /// Clear all tasks from queue
    pub fn clear(&self) {
        self.high_priority.clear();
        self.normal_priority.clear();
        self.low_priority.clear();

        // Reset counters
        self.queue_sizes[0].store(0, Ordering::Relaxed);
        self.queue_sizes[1].store(0, Ordering::Relaxed);
        self.queue_sizes[2].store(0, Ordering::Relaxed);

        // Drain channel
        while self.task_receiver.try_recv().is_ok() {}
    }

    /// Remove expired tasks from all queues
    pub fn cleanup_expired_tasks(&self) -> usize {
        let now = Instant::now();
        let mut removed_count = 0;

        // Clean high priority queue
        let expired_high: Vec<_> = self
            .high_priority
            .iter()
            .filter(|entry| entry.value().deadline < now)
            .map(|entry| *entry.key())
            .collect();

        for key in expired_high {
            if self.high_priority.remove(&key).is_some() {
                self.queue_sizes[0].fetch_sub(1, Ordering::Relaxed);
                removed_count += 1;
            }
        }

        // Clean normal priority queue
        let expired_normal: Vec<_> = self
            .normal_priority
            .iter()
            .filter(|entry| entry.value().deadline < now)
            .map(|entry| *entry.key())
            .collect();

        for key in expired_normal {
            if self.normal_priority.remove(&key).is_some() {
                self.queue_sizes[1].fetch_sub(1, Ordering::Relaxed);
                removed_count += 1;
            }
        }

        // Clean low priority queue
        let expired_low: Vec<_> = self
            .low_priority
            .iter()
            .filter(|entry| entry.value().deadline < now)
            .map(|entry| *entry.key())
            .collect();

        for key in expired_low {
            if self.low_priority.remove(&key).is_some() {
                self.queue_sizes[2].fetch_sub(1, Ordering::Relaxed);
                removed_count += 1;
            }
        }

        removed_count
    }

    /// Get queue statistics for monitoring
    pub fn get_queue_stats(&self) -> QueueStatistics {
        let (high, normal, low) = self.get_queue_sizes();

        QueueStatistics {
            high_priority_count: high,
            normal_priority_count: normal,
            low_priority_count: low,
            total_count: high + normal + low,
            channel_pending: self.task_receiver.len(),
        }
    }
}

/// Queue statistics for monitoring and debugging
#[derive(Debug, Clone)]
pub struct QueueStatistics {
    pub high_priority_count: usize,
    pub normal_priority_count: usize,
    pub low_priority_count: usize,
    pub total_count: usize,
    pub channel_pending: usize,
}

impl PromotionPriority {
    /// Create new promotion priority with current timestamp
    pub fn new(level: u8) -> Self {
        Self {
            level,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
        }
    }

    /// Create promotion priority with specific timestamp
    pub fn with_timestamp(level: u8, timestamp: u64) -> Self {
        Self { level, timestamp }
    }
}
