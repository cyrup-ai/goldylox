//! Core types and data structures for tier promotion management
//!
//! This module defines the main data structures used for intelligent tier
//! promotion and demotion with SIMD-optimized criteria and lock-free queues.

use std::sync::atomic::{AtomicU32, AtomicU64};
use std::time::Instant;

use crossbeam_skiplist::SkipMap;
use crossbeam_utils::CachePadded;

pub use crate::cache::traits::TierLocation;

/// Tier promotion/demotion manager
#[derive(Debug)]
pub struct TierPromotionManager {
    /// Promotion criteria
    pub promotion_criteria: PromotionCriteria,
    /// Demotion criteria
    pub demotion_criteria: DemotionCriteria,
    /// Promotion statistics
    pub promotion_stats: PromotionStatistics,
    /// Promotion task queue
    pub promotion_queue: PromotionQueue,
}

/// SIMD-optimized promotion criteria for cache tier advancement
#[derive(Debug)]
#[repr(align(64))]
pub struct PromotionCriteria {
    /// Frequency thresholds for promotion (SIMD-aligned for parallel comparison)
    pub frequency_thresholds: CachePadded<[AtomicU32; 8]>,
    /// Recency weights for scoring (AVX2-optimized)
    pub recency_weights: [f32; 8],
    /// Minimum access count for promotion consideration
    pub min_access_count: AtomicU32,
    /// Promotion frequency multiplier
    pub frequency_multiplier: AtomicU32,
    /// SIMD computation buffer (AVX2-aligned)
    pub simd_score_buffer: [u32; 8],
}

/// SIMD-optimized demotion criteria for cache tier demotion decisions
#[derive(Debug)]
#[repr(align(64))]
pub struct DemotionCriteria {
    /// Cache pressure thresholds (SIMD-parallel evaluation)
    pub pressure_thresholds: CachePadded<[AtomicU32; 4]>,
    /// Age-based demotion weights
    pub age_weights: [f32; 8],
    /// Maximum idle time before demotion (nanoseconds)
    pub max_idle_time: AtomicU64,
    /// Memory pressure trigger threshold
    pub memory_pressure_threshold: AtomicU32,
    /// SIMD computation buffer for parallel scoring
    pub simd_pressure_buffer: [u32; 8],
}

/// Lock-free promotion statistics with atomic counters
#[derive(Debug)]
#[repr(align(64))]
pub struct PromotionStatistics {
    /// Promotion counts per tier transition
    pub hot_to_warm_promotions: AtomicU64,
    pub warm_to_hot_promotions: AtomicU64,
    pub cold_to_warm_promotions: AtomicU64,
    pub warm_to_cold_demotions: AtomicU64,
    /// Average promotion decision time (nanoseconds)
    pub avg_decision_time: AtomicU64,
    /// Total operations processed
    pub total_operations: AtomicU64,
    /// Success rate of promotion decisions (x1000 for precision)
    pub success_rate_x1000: AtomicU32,
    /// SIMD operations count
    pub simd_operations_count: AtomicU64,
}

/// Lock-free promotion queue using crossbeam-skiplist for ordered operations
#[derive(Debug)]
pub struct PromotionQueue {
    /// Priority-ordered promotion tasks
    pub task_queue: SkipMap<PromotionPriority, PromotionTask<K>>,
    /// Queue statistics
    pub queue_size: AtomicU32,
    pub max_queue_size: AtomicU32,
    /// Processing statistics
    pub tasks_processed: AtomicU64,
    pub tasks_dropped: AtomicU64,
    /// Task processing time tracking
    pub total_processing_time: AtomicU64,
}

/// Promotion task priority for skiplist ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PromotionPriority {
    /// Urgency level (higher = more urgent)
    pub urgency: u16,
    /// Timestamp for tie-breaking (earlier = higher priority)
    pub timestamp_ns: u64,
    /// Task sequence number for deterministic ordering
    pub sequence: u32,
}

/// Promotion task definition
#[derive(Debug, Clone)]
pub struct PromotionTask<K: crate::cache::traits::CacheKey> {
    /// Cache key to promote/demote
    pub key: K,
    /// Source tier
    pub source_tier: TierLocation,
    /// Target tier
    pub target_tier: TierLocation,
    /// Task type (promotion or demotion)
    pub task_type: PromotionTaskType,
    /// Priority score for scheduling
    pub priority_score: u32,
    /// Creation timestamp
    pub created_at: Instant,
}

/// Promotion task type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionTaskType {
    /// Promote entry to higher tier
    Promote,
    /// Demote entry to lower tier
    Demote,
    /// Emergency promotion due to high access rate
    EmergencyPromote,
    /// Forced demotion due to memory pressure
    ForcedDemote,
}

// TierLocation moved to canonical location: crate::cache::traits::types_and_enums
