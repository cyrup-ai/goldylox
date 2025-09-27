//! Core types and data structures for cache policy engine
//!
//! This module defines the main data structures used for advanced cache
//! policy decisions including replacement algorithms, write policies, and
//! machine learning-based prefetch prediction.

#![allow(dead_code)] // Cache policies - intelligent replacement and ML-based prediction algorithms

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use crossbeam_utils::{CachePadded, atomic::AtomicCell};

pub(crate) use crate::cache::traits::AccessType;

/// Multi-algorithm cache replacement policies with machine learning adaptation
#[derive(Debug)]
pub struct ReplacementPolicies<K: crate::cache::traits::CacheKey> {
    /// Current active replacement algorithm
    pub active_algorithm: AtomicCell<ReplacementAlgorithm>,
    /// Algorithm performance metrics for adaptive selection
    pub algorithm_metrics: CachePadded<[AlgorithmMetrics; 4]>,
    /// Access pattern weights for ML-based decisions
    pub pattern_weights: CachePadded<[AtomicU32; 16]>, // SIMD-aligned weights
    /// Temporal access history for ML regression
    pub temporal_history: LockFreeCircularBuffer<TemporalAccess>,
    /// Algorithm switching cooldown timer
    pub last_algorithm_switch: AtomicCell<Instant>,
    /// Performance evaluation period
    pub evaluation_period: Duration,
    /// SIMD computation buffers (AVX2-aligned)
    pub simd_score_buffer: [f32; 8],
    pub simd_weight_buffer: [f32; 8],
    /// Real LRU tracker implementation
    pub lru_tracker: crate::cache::tier::warm::eviction::ConcurrentLruTracker<K>,
    /// Real LFU tracker implementation  
    pub lfu_tracker: crate::cache::tier::warm::eviction::ConcurrentLfuTracker<K>,
    /// Real ARC eviction state implementation
    pub arc_state: crate::cache::tier::warm::eviction::ArcEvictionState<K>,
    /// Real ML eviction policy implementation
    pub ml_policy: crate::cache::tier::warm::eviction::MachineLearningEvictionPolicy<K>,
}

// WritePolicyManager moved to canonical location: crate::cache::eviction::write_policies::WritePolicyManager
// Use the enhanced canonical implementation with comprehensive async write-behind processing,
// complete batching configuration, flush coordination, detailed statistics tracking,
// and production-ready error handling with channels and background processing

// PrefetchPredictor moved to canonical location: crate::cache::tier::hot::prefetch::core::PrefetchPredictor
// Use the enhanced canonical implementation with comprehensive pattern detection workflow,
// performance-optimized ML regression, atomic operations, and SIMD optimizations

/// Replacement algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementAlgorithm {
    Lru,
    Lfu,
    Arc,
    MLBased,
}

/// Write policy types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePolicy {
    WriteThrough,
    WriteBack,
    WriteAround,
    Adaptive,
}

/// Algorithm performance metrics
#[derive(Debug)]
pub struct AlgorithmMetrics {
    pub hit_count: AtomicU64,
    pub miss_count: AtomicU64,
    pub eviction_count: AtomicU64,
    pub avg_access_time: AtomicU64,
}

/// Temporal access pattern for ML analysis
#[derive(Debug, Clone)]
pub struct TemporalAccess {
    pub timestamp: Instant,
    pub key_hash: u64,
    pub access_type: AccessType,
    pub tier: u8,
}

// AccessType moved to canonical location: crate::cache::traits::types_and_enums

/// Dirty entry tracking for write-back policy
#[derive(Debug, Clone)]
pub struct DirtyEntry {
    pub last_modified: Instant,
    pub modification_count: u32,
    pub write_priority: u8,
    pub size_bytes: u32,
}

/// Coalesced write operation
#[derive(Debug, Clone)]
pub struct CoalescedWrite<K: crate::cache::traits::CacheKey> {
    pub keys: Vec<K>,
    pub total_size: u32,
    pub priority: u8,
    pub deadline: Instant,
}

/// Write scheduler for batched operations
#[derive(Debug)]
pub struct WriteScheduler<K: crate::cache::traits::CacheKey> {
    pub pending_writes: AtomicU32,
    pub batch_size_threshold: AtomicU32,
    pub batch_timeout_ns: AtomicU64,
    pub last_batch_time: AtomicCell<Instant>,
    pub _phantom: std::marker::PhantomData<K>,
}

// ThroughputMetrics moved to canonical location: crate::cache::tier::warm::metrics::ThroughputMetrics
// Use the canonical implementation which includes all write metrics plus advanced features like peak tracking and efficiency ratios

/// Adaptive policy configuration
#[derive(Debug)]
pub struct AdaptivePolicyConfig {
    pub ml_prediction_active: AtomicBool,
    pub adaptation_interval: Duration,
    pub performance_threshold: AtomicU32,
    pub learning_rate: AtomicCell<f32>,
}

/// Background write coordinator
#[derive(Debug)]
pub struct BackgroundWriteCoordinator {
    pub active_writers: AtomicU32,
    pub max_writers: AtomicU32,
    pub write_queue_size: AtomicU32,
    pub coordinator_status: AtomicCell<CoordinatorStatus>,
}

// Pattern analysis types moved to canonical location: crate::cache::tier::hot::prefetch::types
// Use the canonical types from the hot tier prefetch module
pub use crate::cache::tier::hot::prefetch::types::AccessSequence;

/// Coordinator status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorStatus {
    Active,
    Idle,
    Overloaded,
    Error,
}

// Lock-free data structures (real crossbeam implementations)
#[derive(Debug)]
pub struct LockFreeCircularBuffer<T> {
    queue: ArrayQueue<T>,
}

impl<T> LockFreeCircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: ArrayQueue::new(capacity),
        }
    }

    pub fn push(&self, item: T) -> Result<(), T> {
        self.queue.push(item)
    }

    pub fn pop(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[derive(Debug)]
pub struct LockFreeQueue<T> {
    queue: ArrayQueue<T>,
}

impl<T> Default for LockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LockFreeQueue<T> {
    pub fn new() -> Self {
        Self {
            queue: ArrayQueue::new(1024), // Default capacity
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: ArrayQueue::new(capacity),
        }
    }

    pub fn push(&self, item: T) -> Result<(), T> {
        self.queue.push(item)
    }

    pub fn pop(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
