//! Core types and data structures for eviction policies
//!
//! This module defines the fundamental types, enums, and traits used across
//! all eviction policy implementations.

use std::sync::atomic::AtomicU64;

use crossbeam_utils::atomic::AtomicCell;

pub use crate::cache::traits::{AccessType, EvictionReason};

/// Eviction policy types for cache management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicyType {
    /// Least Recently Used policy
    Lru,
    /// Least Frequently Used policy  
    Lfu,
    /// Adaptive Replacement Cache policy
    Arc,
    /// Time-based TTL eviction
    Ttl,
    /// Adaptive policy that switches based on performance
    Adaptive,
    /// Random eviction (for testing/fallback)
    Random,
    /// Size-based eviction for memory pressure
    SizeBased,
    /// Cost-aware eviction considering computation cost
    CostAware,
    /// Machine learning-based eviction
    MachineLearning,
}

/// Eviction policy performance metrics
#[derive(Debug, Clone, Copy)]
pub struct PolicyPerformanceMetrics {
    /// Hit rate for this policy
    pub hit_rate: f64,
    /// Average access time in nanoseconds
    pub avg_access_time_ns: u64,
    /// Eviction efficiency score
    pub eviction_efficiency: f64,
}

impl Default for PolicyPerformanceMetrics {
    fn default() -> Self {
        Self {
            hit_rate: 0.5,
            avg_access_time_ns: 1000,
            eviction_efficiency: 0.5,
        }
    }
}

/// Frequency statistics for global tracking
#[derive(Debug, Default)]
pub struct FrequencyStats {
    /// Total accesses across all keys
    pub total_accesses: AtomicU64,
    /// Number of unique keys
    pub unique_keys: AtomicU64,
    /// Average frequency
    pub avg_frequency: AtomicCell<f64>,
}

/// Frequency trend indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyTrend {
    /// Frequency is increasing
    Increasing,
    /// Frequency is decreasing
    Decreasing,
    /// Frequency is stable
    Stable,
}

impl Default for FrequencyTrend {
    fn default() -> Self {
        Self::Stable
    }
}

// AccessType moved to canonical location: crate::cache::traits::types_and_enums

/// LRU tracking statistics
#[derive(Debug)]
pub struct LruStats {
    /// Total access count
    pub total_accesses: crossbeam_utils::CachePadded<AtomicU64>,
    /// Number of LRU evictions
    pub lru_evictions: crossbeam_utils::CachePadded<AtomicU64>,
    /// Average access age in nanoseconds
    pub avg_access_age_ns: AtomicCell<f64>,
}

impl Default for LruStats {
    fn default() -> Self {
        Self {
            total_accesses: crossbeam_utils::CachePadded::new(AtomicU64::new(0)),
            lru_evictions: crossbeam_utils::CachePadded::new(AtomicU64::new(0)),
            avg_access_age_ns: AtomicCell::new(0.0),
        }
    }
}

/// LFU tracking statistics
#[derive(Debug)]
pub struct LfuStats {
    /// Total frequency updates
    pub frequency_updates: AtomicU64,
    /// Number of LFU evictions
    pub lfu_evictions: AtomicU64,
    /// Average frequency across all keys
    pub avg_frequency: AtomicCell<f64>,
}

impl Default for LfuStats {
    fn default() -> Self {
        Self {
            frequency_updates: AtomicU64::new(0),
            lfu_evictions: AtomicU64::new(0),
            avg_frequency: AtomicCell::new(1.0),
        }
    }
}

/// ARC adaptation statistics
#[derive(Debug)]
pub struct ArcStats {
    /// T1 hits
    pub t1_hits: AtomicU64,
    /// T2 hits
    pub t2_hits: AtomicU64,
    /// B1 ghost hits
    pub b1_ghost_hits: AtomicU64,
    /// B2 ghost hits
    pub b2_ghost_hits: AtomicU64,
    /// Adaptation parameter changes
    pub adaptations: AtomicU64,
}

impl Default for ArcStats {
    fn default() -> Self {
        Self {
            t1_hits: AtomicU64::new(0),
            t2_hits: AtomicU64::new(0),
            b1_ghost_hits: AtomicU64::new(0),
            b2_ghost_hits: AtomicU64::new(0),
            adaptations: AtomicU64::new(0),
        }
    }
}

/// Machine learning model statistics
#[derive(Debug)]
pub struct MlStats {
    /// Number of predictions made
    pub predictions: AtomicU64,
    /// Number of correct predictions
    pub correct_predictions: AtomicU64,
    /// Model training iterations
    pub training_iterations: AtomicU64,
    /// Current model accuracy
    pub accuracy: AtomicCell<f64>,
}

impl Default for MlStats {
    fn default() -> Self {
        Self {
            predictions: AtomicU64::new(0),
            correct_predictions: AtomicU64::new(0),
            training_iterations: AtomicU64::new(0),
            accuracy: AtomicCell::new(0.5),
        }
    }
}

/// Eviction candidate with score
#[derive(Debug, Clone)]
pub struct EvictionCandidate<K> {
    /// Cache key
    pub key: K,
    /// Eviction score (higher = more likely to evict)
    pub score: f64,
    /// Reason for eviction
    pub reason: EvictionReason,
}

// EvictionReason moved to canonical location: crate::cache::traits::types_and_enums

impl<K> EvictionCandidate<K> {
    /// Create new eviction candidate
    pub fn new(key: K, score: f64, reason: EvictionReason) -> Self {
        Self { key, score, reason }
    }
}

/// Eviction policy trait
pub trait EvictionPolicy<K> {
    /// Record access event
    fn on_access(&self, key: &K, hit: bool);

    /// Record eviction event
    fn on_eviction(&self, key: &K);

    /// Select eviction candidates
    fn select_candidates(&self, count: usize) -> Vec<K>;

    /// Get policy performance metrics
    fn performance_metrics(&self) -> PolicyPerformanceMetrics;

    /// Calculate average access time in nanoseconds
    fn calculate_average_access_time_ns(&self) -> u64;

    /// Adapt policy based on performance
    fn adapt(&self);
}

/// Eviction policy factory
pub trait EvictionPolicyFactory<K> {
    /// Create new eviction policy instance
    fn create_policy(&self, config: &EvictionConfig) -> Box<dyn EvictionPolicy<K>>;
}

/// Eviction policy configuration
#[derive(Debug, Clone)]
pub struct EvictionConfig {
    /// Maximum cache size
    pub max_size: u64,
    /// Adaptation enabled
    pub adaptive: bool,
    /// Machine learning enabled
    pub ml_enabled: bool,
    /// Statistics collection enabled
    pub stats_enabled: bool,
}

impl Default for EvictionConfig {
    fn default() -> Self {
        Self {
            max_size: 1024,
            adaptive: true,
            ml_enabled: false,
            stats_enabled: true,
        }
    }
}
