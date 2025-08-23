//! Core types and data structures for eviction system
//!
//! This module defines the fundamental types used throughout the eviction
//! system including policies, candidates, and configuration structures.

use std::time::Duration;

pub use crate::cache::traits::{AccessType, EvictionReason};

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

/// Eviction candidate with scoring
#[derive(Debug, Clone)]
pub struct EvictionCandidate {
    pub slot_index: usize,
    pub score: f64,
    pub reason: EvictionReason,
}

// EvictionReason moved to canonical location: crate::cache::traits::types_and_enums

/// Access event for pattern learning
#[derive(Debug, Clone)]
pub struct AccessEvent {
    pub timestamp: u64,
    pub slot_index: usize,
    pub access_type: AccessType,
    pub hit: bool,
}

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

/// Eviction configuration
#[derive(Debug, Clone)]
pub struct EvictionConfig {
    pub default_policy: EvictionPolicy,
    pub learning_enabled: bool,
    pub history_size: usize,
    pub adaptation_interval: Duration,
    pub performance_threshold: f64,
}

/// Eviction statistics
#[derive(Debug, Clone)]
pub struct EvictionStats {
    pub policy: EvictionPolicy,
    pub total_evictions: u64,
    pub hit_rate: f64,
    pub avg_eviction_time_ns: u64,
    pub feature_weights: FeatureWeights,
    pub learning_enabled: bool,
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

        // Keep learning enabled if either has it enabled
        self.learning_enabled = self.learning_enabled || other.learning_enabled;

        // Feature weights remain from current (could be averaged in full implementation)
    }
}

impl Default for EvictionConfig {
    fn default() -> Self {
        Self {
            default_policy: EvictionPolicy::Lru,
            learning_enabled: true,
            history_size: 1000,
            adaptation_interval: Duration::from_secs(60),
            performance_threshold: 0.8,
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
            learning_enabled: false,
        }
    }
}
