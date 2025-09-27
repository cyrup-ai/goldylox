#![allow(dead_code)]
// Hot tier eviction types - Complete eviction type library with policies, candidates, feature weights, and performance tracking

//! Core types and data structures for eviction system
//!
//! This module defines the fundamental types used throughout the eviction
//! system including policies, candidates, and configuration structures.

pub(crate) use crate::cache::traits::AccessType;

/// Eviction policy types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvictionPolicy {
    /// Least Recently Used
    Lru,
    /// Least Frequently Used  
    Lfu,
    /// Adaptive Replacement Cache
    Arc,
    /// Machine Learning based
    MachineLearning,
}

// EvictionCandidate moved to canonical location: crate::cache::types::eviction::candidate::EvictionCandidate
pub use crate::cache::types::eviction::candidate::EvictionCandidate;

/// Hot tier specific constructor alias
pub fn create_hot_tier_candidate<
    K: crate::cache::traits::CacheKey,
    V: crate::cache::traits::CacheValue,
>(
    slot_index: usize,
    key: K,
    score: f64,
    _reason: crate::cache::traits::types_and_enums::EvictionReason,
) -> EvictionCandidate<K, V> {
    use crate::cache::traits::types_and_enums::SelectionReason;
    EvictionCandidate::from_slot_index(slot_index, key, score, SelectionReason::LeastRecentlyUsed)
}

// EvictionReason moved to canonical location: crate::cache::traits::types_and_enums

// AccessEvent moved to canonical location: crate::cache::eviction::types::AccessEvent
pub use crate::cache::eviction::types::AccessEvent;

// AccessType moved to canonical location: crate::cache::traits::types_and_enums

/// Feature weights for machine learning eviction
#[derive(Debug, Clone)]
pub struct FeatureWeights {
    pub recency_weight: f64,
    pub frequency_weight: f64,
    pub size_weight: f64,
    pub utility_weight: f64,
    pub temporal_weight: f64,
}

/// Eviction performance metrics
#[derive(Debug, Default)]
pub struct EvictionMetrics {
    pub total_evictions: u64,
    pub correct_evictions: u64,
    pub false_evictions: u64,
    pub eviction_time_ns: u64,
    pub hit_rate_improvement: f64,
}

// Re-export canonical EvictionConfig from warm tier
pub use crate::cache::tier::warm::config::HotTierEvictionConfig;
use crate::cache::tier::warm::eviction::types::EvictionPolicyType;

/// Eviction statistics
#[derive(Debug, Clone)]
pub struct EvictionStats {
    #[allow(dead_code)] // ML system - used in hot tier eviction policy statistics
    pub policy: EvictionPolicy,
    #[allow(dead_code)] // ML system - used in hot tier eviction policy statistics
    pub total_evictions: u64,
    #[allow(dead_code)] // ML system - used in hot tier eviction policy statistics
    pub hit_rate: f64,
    #[allow(dead_code)] // ML system - used in hot tier eviction policy statistics
    pub avg_eviction_time_ns: u64,
    #[allow(dead_code)] // ML system - used in hot tier eviction policy statistics
    pub feature_weights: FeatureWeights,
}

impl EvictionStats {
    /// Merge statistics from another EvictionStats
    pub fn merge(&mut self, other: EvictionStats) {
        // Keep the same policy (prefer current)
        self.total_evictions += other.total_evictions;

        // Average the hit rates
        self.hit_rate = (self.hit_rate + other.hit_rate) / 2.0;

        // Average the eviction times
        self.avg_eviction_time_ns = (self.avg_eviction_time_ns + other.avg_eviction_time_ns) / 2;

        // Feature weights remain from current (could be averaged in full implementation)
    }
}

impl From<EvictionPolicyType> for EvictionPolicy {
    fn from(policy_type: EvictionPolicyType) -> Self {
        match policy_type {
            EvictionPolicyType::Lru => EvictionPolicy::Lru,
            EvictionPolicyType::Lfu => EvictionPolicy::Lfu,
            EvictionPolicyType::Arc => EvictionPolicy::Arc,
            // Map all advanced policies to MachineLearning (hot tier fallback)
            EvictionPolicyType::Adaptive
            | EvictionPolicyType::Ttl
            | EvictionPolicyType::Random
            | EvictionPolicyType::SizeBased
            | EvictionPolicyType::CostAware
            | EvictionPolicyType::MachineLearning
            | EvictionPolicyType::Fifo
            | EvictionPolicyType::Clock
            | EvictionPolicyType::Lru2 => EvictionPolicy::MachineLearning,
        }
    }
}

impl Default for FeatureWeights {
    fn default() -> Self {
        Self {
            recency_weight: 0.4,
            frequency_weight: 0.3,
            size_weight: 0.1,
            utility_weight: 0.15,
            temporal_weight: 0.05,
        }
    }
}

impl EvictionStats {
    /// Check if eviction performance is good
    pub fn is_performing_well(&self, threshold: f64) -> bool {
        self.hit_rate >= threshold
    }

    /// Get eviction efficiency score
    pub fn efficiency_score(&self) -> f64 {
        if self.avg_eviction_time_ns > 0 {
            self.hit_rate / (self.avg_eviction_time_ns as f64 / 1_000_000.0) // Normalize by milliseconds
        } else {
            self.hit_rate
        }
    }
}

impl Default for EvictionStats {
    fn default() -> Self {
        Self {
            policy: EvictionPolicy::Lru,
            total_evictions: 0,
            hit_rate: 0.0,
            avg_eviction_time_ns: 0,
            feature_weights: FeatureWeights::default(),
        }
    }
}
