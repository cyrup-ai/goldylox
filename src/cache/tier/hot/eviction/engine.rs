//! Main eviction engine implementation
//!
//! This module implements the adaptive eviction engine with machine learning
//! capabilities and policy management.

use super::super::memory_pool::SlotMetadata;
use super::super::synchronization::SimdLruTracker;
use super::types::{
    AccessEvent, AccessType, EvictionCandidate, EvictionConfig, EvictionMetrics, EvictionPolicy,
    EvictionStats, FeatureWeights,
};

/// Adaptive eviction engine with machine learning
#[derive(Debug)]
pub struct EvictionEngine {
    /// Current eviction policy
    pub policy: EvictionPolicy,
    /// Access pattern history for learning
    pub access_history: Vec<AccessEvent>,
    /// Feature weights for ML-based eviction
    pub feature_weights: FeatureWeights,
    /// Performance metrics for adaptive tuning
    pub performance_metrics: EvictionMetrics,
    /// Configuration
    pub config: EvictionConfig,
}

impl EvictionEngine {
    /// Create new eviction engine
    pub fn new(config: EvictionConfig) -> Self {
        Self {
            policy: config.default_policy,
            access_history: Vec::with_capacity(config.history_size),
            feature_weights: FeatureWeights::default(),
            performance_metrics: EvictionMetrics::default(),
            config,
        }
    }

    /// Find best eviction candidate using SIMD-accelerated search
    pub fn find_eviction_candidate(
        &mut self,
        metadata: &[SlotMetadata; 256],
        lru_tracker: &SimdLruTracker,
        current_time_ns: u64,
    ) -> Option<EvictionCandidate> {
        match self.policy {
            EvictionPolicy::Lru => self.find_lru_candidate(metadata, lru_tracker),
            EvictionPolicy::Lfu => self.find_lfu_candidate(metadata),
            EvictionPolicy::Arc => self.find_arc_candidate(metadata, lru_tracker, current_time_ns),
            EvictionPolicy::MachineLearning => {
                self.find_ml_candidate(metadata, lru_tracker, current_time_ns)
            }
        }
    }

    /// Record cache access for learning
    pub fn record_access(&mut self, slot_idx: usize, hit: bool, timestamp_ns: u64) {
        if self.config.learning_enabled {
            let event = AccessEvent {
                timestamp: timestamp_ns,
                slot_index: slot_idx,
                access_type: AccessType::Read,
                hit,
            };

            self.access_history.push(event);

            // Limit history size
            if self.access_history.len() > self.config.history_size {
                self.access_history.remove(0);
            }

            // Update metrics
            if hit {
                self.performance_metrics.correct_evictions += 1;
            } else {
                self.performance_metrics.false_evictions += 1;
            }
        }
    }

    /// Record eviction operation
    pub fn record_eviction(&mut self, _slot_idx: usize, success: bool, duration_ns: u64) {
        self.performance_metrics.total_evictions += 1;
        self.performance_metrics.eviction_time_ns += duration_ns;

        if success {
            self.performance_metrics.correct_evictions += 1;
        }

        // Adapt weights based on performance
        if self.performance_metrics.total_evictions % 100 == 0 {
            self.adapt_weights();
        }
    }

    /// Get eviction statistics
    pub fn get_stats(&self) -> EvictionStats {
        let hit_rate = if self.performance_metrics.total_evictions > 0 {
            self.performance_metrics.correct_evictions as f64
                / self.performance_metrics.total_evictions as f64
        } else {
            0.0
        };

        let avg_eviction_time = if self.performance_metrics.total_evictions > 0 {
            self.performance_metrics.eviction_time_ns / self.performance_metrics.total_evictions
        } else {
            0
        };

        EvictionStats {
            policy: self.policy,
            total_evictions: self.performance_metrics.total_evictions,
            hit_rate,
            avg_eviction_time_ns: avg_eviction_time,
            feature_weights: self.feature_weights.clone(),
            learning_enabled: self.config.learning_enabled,
        }
    }

    /// Switch eviction policy
    pub fn set_policy(&mut self, policy: EvictionPolicy) {
        self.policy = policy;

        // Reset metrics when switching policy
        self.performance_metrics = EvictionMetrics::default();
    }

    /// Get current policy
    pub fn policy(&self) -> EvictionPolicy {
        self.policy
    }
}
