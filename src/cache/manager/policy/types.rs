//! Core types and data structures for cache policy engine
//!
//! This module defines the main data structures used for advanced cache
//! policy decisions including replacement algorithms, write policies, and
//! machine learning-based prefetch prediction.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::time::{Duration, Instant};

use crossbeam_queue::ArrayQueue;
use crossbeam_utils::{atomic::AtomicCell, CachePadded};
use dashmap::DashMap;

pub use crate::cache::traits::AccessType;

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

/// Cache write policy manager with intelligent dirty tracking
#[derive(Debug)]
pub struct WritePolicyManager<K: crate::cache::traits::CacheKey> {
    /// Write policies per cache tier
    pub tier_policies: CachePadded<[AtomicCell<WritePolicy>; 3]>, // Hot, Warm, Cold
    /// Dirty bit tracking for write-back entries
    pub dirty_tracker: DashMap<K, DirtyEntry>,
    /// Write coalescing buffer for batched operations
    pub write_coalescing_buffer: LockFreeQueue<CoalescedWrite<K>>,
    /// Write scheduling based on access patterns
    pub write_scheduler: WriteScheduler<K>,
    /// Write throughput metrics per tier
    pub throughput_metrics: CachePadded<[ThroughputMetrics; 3]>,
    /// Adaptive policy configuration
    pub adaptive_config: AdaptivePolicyConfig,
    /// Background write coordinator
    pub background_writer: BackgroundWriteCoordinator,
}

/// Machine learning-based prefetch predictor with polynomial regression
#[derive(Debug)]
pub struct PrefetchPredictor {
    /// Lock-free circular buffer for access sequence history
    pub access_sequence_buffer: LockFreeCircularBuffer<AccessSequence>,
    /// Polynomial regression coefficients (updated via atomic swaps)
    pub regression_coefficients: CachePadded<[AtomicCell<f32>; 8]>,
    /// Prediction confidence scores per pattern type
    pub confidence_scores: CachePadded<[AtomicU32; 4]>, // Sequential, Temporal, Spatial, Random
    /// SIMD-optimized prediction computation buffers
    pub prediction_buffer: [f32; 16], // AVX2-aligned for parallel computation
    pub feature_buffer: [f32; 16], // Feature extraction buffer
    /// Prefetch success rate tracking
    pub success_tracker: PrefetchSuccessTracker,
    /// Adaptive learning rate for online training
    pub learning_rate: AtomicCell<f32>,
    /// Pattern correlation matrix for complex predictions
    pub correlation_matrix: CachePadded<[[AtomicU32; 4]; 4]>,
    /// Prefetch queue for predicted access targets
    pub prefetch_queue: LockFreeQueue<PrefetchTarget>,
}

/// Replacement algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementAlgorithm {
    LRU,
    LFU,
    ARC,
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

/// Throughput metrics per tier
#[derive(Debug)]
pub struct ThroughputMetrics {
    pub writes_per_second: AtomicU32,
    pub bytes_per_second: AtomicU64,
    pub avg_write_latency: AtomicU64,
    pub write_queue_depth: AtomicU32,
}

/// Adaptive policy configuration
#[derive(Debug)]
pub struct AdaptivePolicyConfig {
    pub enable_ml_prediction: AtomicBool,
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

/// Access sequence for pattern analysis
#[derive(Debug, Clone)]
pub struct AccessSequence {
    pub sequence_id: u64,
    pub keys: Vec<u64>,       // Key hashes
    pub timestamps: Vec<u64>, // Nanosecond timestamps
    pub pattern_type: PatternType,
}

/// Pattern type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternType {
    Sequential,
    Random,
    Temporal,
    Spatial,
}

/// Prefetch success tracking
#[derive(Debug)]
pub struct PrefetchSuccessTracker {
    pub predictions_made: AtomicU64,
    pub predictions_hit: AtomicU64,
    pub false_positives: AtomicU64,
    pub avg_prediction_accuracy: AtomicCell<f32>,
}

/// Prefetch target
#[derive(Debug, Clone)]
pub struct PrefetchTarget {
    pub key_hash: u64,
    pub confidence: f32,
    pub predicted_access_time: Instant,
    pub priority: u8,
}

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
