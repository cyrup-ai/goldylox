//! Core types and data structures for write propagation system
//!
//! This module defines the fundamental types used throughout the write
//! propagation system for cache coherence.

use std::sync::atomic::{AtomicU32, AtomicU64};
use std::time::Instant;

use crossbeam_skiplist::SkipMap;

use super::worker_system::WorkerChannels;
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey};
use crate::cache::traits::{CacheKey, CacheValue};

/// Write propagation system for data consistency
#[derive(Debug)]
pub struct WritePropagationSystem<K: CacheKey, V: CacheValue> {
    /// Write-back queue
    pub writeback_queue: SkipMap<u64, WriteBackRequest<K, V>>,
    /// Write propagation policy (atomic for safe runtime updates)
    pub propagation_policy: std::sync::atomic::AtomicU8, // Encoded PropagationPolicy
    /// Background worker channels
    pub worker_channels: WorkerChannels<K, V>,
    /// Propagation statistics
    pub propagation_stats: PropagationStatistics,
    /// System configuration
    pub config: PropagationConfig,
    /// Worker health monitoring
    #[allow(dead_code)]
    // Background workers - used in async task processing and worker coordination
    pub worker_health: AtomicU32, // Encoded WorkerHealth
    /// Semaphore to limit concurrent write propagation tasks (prevents runtime overwhelming)
    pub write_semaphore: std::sync::Arc<tokio::sync::Semaphore>,
    /// Hot tier coordinator for tier operations
    pub hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
    /// Warm tier coordinator for tier operations
    pub warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
    /// Cold tier coordinator for tier operations
    pub cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
}

/// Write-back request for background processing
#[derive(Debug, Clone)]
pub struct WriteBackRequest<K: CacheKey, V: CacheValue> {
    pub key: CoherenceKey<K>,
    pub data: V,
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
    #[allow(dead_code)]
    // MESI coherence - used in adaptive policy configuration and switching logic
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
