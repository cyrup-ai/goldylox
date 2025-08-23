//! Core types and data structures for write propagation system
//!
//! This module defines the fundamental types used throughout the write
//! propagation system for cache coherence.

use std::sync::atomic::{AtomicU32, AtomicU64};
use std::sync::Arc;
use std::time::Instant;

use crossbeam_skiplist::SkipMap;

use super::super::data_structures::{CacheTier, CoherenceKey};
use super::worker_system::WorkerChannels;
use crate::cache::traits::{CacheKey, CacheValue};

/// Write propagation system for data consistency
#[derive(Debug)]
pub struct WritePropagationSystem<K: CacheKey, V: CacheValue> {
    /// Write-back queue
    pub writeback_queue: SkipMap<u64, WriteBackRequest<K, V>>,
    /// Write propagation policy
    pub propagation_policy: PropagationPolicy,
    /// Background worker channels
    pub worker_channels: WorkerChannels<K, V>,
    /// Propagation statistics
    pub propagation_stats: PropagationStatistics,
    /// System configuration
    pub config: PropagationConfig,
    /// Worker health monitoring
    pub worker_health: AtomicU32, // Encoded WorkerHealth
}

/// Write-back request for background processing
#[derive(Debug, Clone)]
pub struct WriteBackRequest<K: CacheKey, V: CacheValue> {
    pub key: CoherenceKey<K>,
    pub data: Arc<V>,
    pub source_tier: CacheTier,
    pub target_tier: CacheTier,
    pub version: u64,
    pub created_at: Instant,
    pub priority: WritePriority,
}

/// Priority levels for write operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WritePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Write propagation policies
#[derive(Debug, Clone, Copy)]
pub enum PropagationPolicy {
    WriteThrough,
    WriteBack,
    Adaptive,
}

/// Configuration for write propagation behavior
#[derive(Debug, Clone)]
pub struct PropagationConfig {
    /// Write-back delay in nanoseconds
    pub writeback_delay_ns: u64,
    /// Maximum queue size
    pub max_queue_size: u32,
    /// Batch size for bulk operations
    pub batch_size: u32,
    /// Enable adaptive policy switching
    pub adaptive_switching: bool,
    /// Worker thread count
    pub worker_threads: u32,
}

/// Comprehensive lock-free performance monitoring for WritePropagationSystem
#[derive(Debug)]
pub struct PropagationStatistics {
    /// Total number of writebacks performed
    pub writebacks: AtomicU64,
    /// Total propagation operations (immediate write-through)
    pub propagations: AtomicU64,
    /// Failed write operations
    pub failed_writes: AtomicU64,
    /// Average write latency in nanoseconds
    pub avg_write_latency_ns: AtomicU64,
    /// Peak queue depth
    pub peak_queue_depth: AtomicU32,
    /// Total bytes written
    pub total_bytes_written: AtomicU64,
    /// Write operations by priority
    pub low_priority_writes: AtomicU64,
    pub normal_priority_writes: AtomicU64,
    pub high_priority_writes: AtomicU64,
    pub critical_priority_writes: AtomicU64,
    /// Background worker statistics
    pub worker_tasks_processed: AtomicU64,
    pub worker_errors: AtomicU64,
}

/// Background worker health status
#[derive(Debug, Clone, Copy)]
pub enum WorkerHealth {
    /// Worker is healthy and processing tasks
    Healthy = 0,
    /// Worker is experiencing temporary issues
    Degraded = 1,
    /// Worker is overloaded but functional
    Overloaded = 2,
    /// Worker has failed and needs restart
    Failed = 3,
}

/// Snapshot of propagation statistics for monitoring
#[derive(Debug, Clone, Copy)]
pub struct PropagationStatisticsSnapshot {
    pub writebacks: u64,
    pub propagations: u64,
    pub failed_writes: u64,
    pub avg_write_latency_ns: u64,
    pub peak_queue_depth: u32,
    pub current_queue_depth: u32,
    pub total_bytes_written: u64,
    pub low_priority_writes: u64,
    pub normal_priority_writes: u64,
    pub high_priority_writes: u64,
    pub critical_priority_writes: u64,
    pub worker_tasks_processed: u64,
    pub worker_errors: u64,
}
