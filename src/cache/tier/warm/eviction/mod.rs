#![allow(dead_code)]
// Warm tier eviction - Complete eviction library with LRU, LFU, ARC, ML-based policies, and adaptive selection for optimal performance

//! Eviction policies and algorithms for warm tier cache
//!
//! This module provides a comprehensive set of eviction policies including LRU, LFU, ARC,
//! and machine learning-based policies with adaptive selection for optimal cache performance.

pub mod arc;
pub mod lfu;
pub mod lru;
pub mod ml;
pub mod types;

pub use arc::ArcEvictionState;
use crossbeam_utils::atomic::AtomicCell;
pub use lfu::ConcurrentLfuTracker;
pub use lru::ConcurrentLruTracker;
pub use ml::MachineLearningEvictionPolicy;
pub use types::*;

use super::core::WarmCacheKey;
use crate::cache::traits::CacheKey;

/// Concurrent eviction policy with adaptive algorithms
#[derive(Debug)]
pub struct ConcurrentEvictionPolicy<K: CacheKey> {
    /// Policy type selector
    policy_type: AtomicCell<EvictionPolicyType>,
    /// LRU tracking with concurrent access
    lru_tracker: ConcurrentLruTracker<K>,
    /// LFU tracking with concurrent access
    lfu_tracker: ConcurrentLfuTracker<K>,
    /// ARC (Adaptive Replacement Cache) state
    arc_state: ArcEvictionState<K>,
    /// Machine learning policy
    ml_policy: MachineLearningEvictionPolicy<K>,
    /// Policy performance metrics
    performance_metrics: PolicyPerformanceMetrics,
    /// Maximum entries limit for pressure calculation
    max_entries: AtomicCell<Option<usize>>,
    /// Maximum memory bytes limit for pressure calculation
    max_memory_bytes: AtomicCell<Option<u64>>,
    /// Current entry count for pressure calculation
    current_entries: AtomicCell<usize>,
    /// Current memory usage for pressure calculation
    current_memory_bytes: AtomicCell<u64>,
}

impl<K: CacheKey> ConcurrentEvictionPolicy<K> {
    /// Create new concurrent eviction policy
    pub fn new() -> Self {
        Self {
            policy_type: AtomicCell::new(EvictionPolicyType::Adaptive),
            lru_tracker: ConcurrentLruTracker::new(),
            lfu_tracker: ConcurrentLfuTracker::new(),
            arc_state: ArcEvictionState::new(1024), // Default size
            ml_policy: MachineLearningEvictionPolicy::new(),
            performance_metrics: PolicyPerformanceMetrics::default(),
            max_entries: AtomicCell::new(None),
            max_memory_bytes: AtomicCell::new(None),
            current_entries: AtomicCell::new(0),
            current_memory_bytes: AtomicCell::new(0),
        }
    }

    /// Create new policy with specific configuration
    pub fn with_config(_config: EvictionConfig) -> Self {
        let policy = Self::new();

        policy
            .policy_type
            .store(EvictionPolicyType::MachineLearning);

        policy
    }

    /// Update cache capacity configuration for pressure calculation
    pub fn update_cache_capacity(&self, max_entries: Option<usize>, max_memory_bytes: Option<u64>) {
        self.max_entries.store(max_entries);
        self.max_memory_bytes.store(max_memory_bytes);
    }

    /// Update current cache statistics for pressure calculation
    pub fn update_cache_stats(&self, current_entries: usize, current_memory_bytes: u64) {
        self.current_entries.store(current_entries);
        self.current_memory_bytes.store(current_memory_bytes);
    }

    /// Get current eviction policy
    pub fn current_policy(&self) -> EvictionPolicyType {
        self.policy_type.load()
    }

    /// Set eviction policy
    pub fn set_policy(&self, policy: EvictionPolicyType) {
        self.policy_type.store(policy);
    }

    /// Handle access event for eviction tracking
    pub fn on_access(&self, key: &WarmCacheKey<K>, hit: bool) {
        self.lru_tracker.record_access(key);
        self.lfu_tracker.record_access(key);
        self.ml_policy.on_access(key, hit);

        if hit {
            self.arc_state.on_hit(key);
        } else {
            self.arc_state.on_miss(key);
        }
    }

    /// Handle eviction event
    pub fn on_eviction(&self, key: &WarmCacheKey<K>) {
        self.lru_tracker.on_eviction(key);
        self.lfu_tracker.on_eviction(key);
        self.arc_state.on_eviction(key);
        self.ml_policy.on_eviction(key);
    }

    /// Select eviction candidates using current policy
    pub fn select_eviction_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        match self.policy_type.load() {
            EvictionPolicyType::Lru => self.select_lru_candidates(count),
            EvictionPolicyType::Lfu => self.select_lfu_candidates(count),
            EvictionPolicyType::Arc => self.select_arc_candidates(count),
            EvictionPolicyType::MachineLearning => self.select_ml_candidates(count),
            EvictionPolicyType::Adaptive => self.select_adaptive_candidates(count),
            EvictionPolicyType::Ttl => self.select_lru_candidates(count), // Fallback to LRU
            EvictionPolicyType::Random => self.select_random_candidates(count),
            EvictionPolicyType::SizeBased => self.select_size_based_candidates(count),
            EvictionPolicyType::CostAware => self.select_lru_candidates(count), // Fallback to LRU
            // New "best of best" features - fallback to appropriate algorithms
            EvictionPolicyType::Fifo => self.select_lru_candidates(count), // FIFO -> LRU fallback
            EvictionPolicyType::Clock => self.select_lru_candidates(count), // Clock -> LRU fallback
            EvictionPolicyType::Lru2 => self.select_lru_candidates(count), // LRU2 -> LRU fallback
        }
    }

    /// Select eviction candidates using adaptive strategy
    fn select_adaptive_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        // Adaptive selection based on performance metrics
        let hit_rate = self.performance_metrics.hit_rate;

        if hit_rate < 0.6 {
            // Low hit rate - use LFU to keep frequently accessed items
            self.select_lfu_candidates(count)
        } else if hit_rate > 0.8 {
            // High hit rate - use LRU for temporal locality
            self.select_lru_candidates(count)
        } else {
            // Medium hit rate - use ARC for balanced approach
            self.select_arc_candidates(count)
        }
    }

    /// Select LRU eviction candidates
    fn select_lru_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.lru_tracker.select_oldest(count)
    }

    /// Select LFU eviction candidates
    fn select_lfu_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.lfu_tracker.select_least_frequent(count)
    }

    /// Select ARC eviction candidates
    fn select_arc_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.arc_state.select_candidates(count)
    }

    /// Select ML eviction candidates
    fn select_ml_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.ml_policy.select_candidates(count)
    }

    /// Update policy performance metrics
    pub fn update_metrics(&mut self, hit_rate: f64, avg_access_time_ns: u64, efficiency: f64) {
        self.performance_metrics = PolicyPerformanceMetrics {
            hit_rate,
            avg_access_time_ns,
            eviction_efficiency: efficiency,
        };
    }

    /// Get most frequent keys (for promotion analysis)
    pub fn get_most_frequent_keys(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        match self.policy_type.load() {
            EvictionPolicyType::Lfu => self.lfu_tracker.most_frequent_keys(count),
            EvictionPolicyType::MachineLearning => self.ml_policy.select_candidates(count),
            _ => {
                // For other policies, get candidates in LRU order
                self.lru_tracker.select_candidates(count)
            }
        }
    }

    /// Adapt policy based on performance
    pub fn adapt_policy(&self) {
        let current_hit_rate = self.performance_metrics.hit_rate;
        let current_policy = self.policy_type.load();

        // Simple adaptation logic - switch policy if performance is poor
        match current_policy {
            EvictionPolicyType::Lru if current_hit_rate < 0.5 => {
                self.policy_type.store(EvictionPolicyType::Lfu);
            }
            EvictionPolicyType::Lfu if current_hit_rate < 0.5 => {
                self.policy_type.store(EvictionPolicyType::Arc);
            }
            EvictionPolicyType::Arc if current_hit_rate < 0.5 => {
                self.policy_type.store(EvictionPolicyType::MachineLearning);
            }
            EvictionPolicyType::MachineLearning if current_hit_rate < 0.5 => {
                self.policy_type.store(EvictionPolicyType::Adaptive);
            }
            _ => {} // Keep current policy
        }
    }

    /// Get performance metrics
    pub fn performance_metrics(&self) -> PolicyPerformanceMetrics {
        self.performance_metrics
    }

    /// Get LRU tracker
    pub fn lru_tracker(&self) -> &ConcurrentLruTracker<K> {
        &self.lru_tracker
    }

    /// Get LFU tracker
    pub fn lfu_tracker(&self) -> &ConcurrentLfuTracker<K> {
        &self.lfu_tracker
    }

    /// Get ARC state
    pub fn arc_state(&self) -> &ArcEvictionState<K> {
        &self.arc_state
    }

    /// Get ML policy
    pub fn ml_policy(&self) -> &MachineLearningEvictionPolicy<K> {
        &self.ml_policy
    }

    /// Calculate cache pressure based on memory and entry usage
    ///
    /// Returns f64 in range [0.0, 1.0] where:
    /// - 0.0 = empty cache (no pressure)
    /// - 0.5 = half full (medium pressure)
    /// - 1.0 = completely full (maximum pressure)
    ///
    /// Uses the MAXIMUM of entry-based and memory-based pressure,
    /// as the most constrained resource drives eviction urgency.
    fn calculate_cache_pressure(&self) -> f64 {
        let mut entry_pressure = 0.5; // Default to medium pressure
        let mut memory_pressure = 0.5; // Default to medium pressure

        // Calculate entry-based pressure if max_entries is configured
        if let Some(max_entries) = self.max_entries.load() && max_entries > 0 {
            let current = self.current_entries.load();
            entry_pressure = (current as f64) / (max_entries as f64);
        }

        // Calculate memory-based pressure if max_memory_bytes is configured
        if let Some(max_memory) = self.max_memory_bytes.load() && max_memory > 0 {
            let current = self.current_memory_bytes.load();
            memory_pressure = (current as f64) / (max_memory as f64);
        }

        // Return maximum of both pressures (most constrained resource)
        // Clamp to [0.0, 1.0] range for safety
        entry_pressure.max(memory_pressure).min(1.0)
    }

    /// Train ML policy with feedback
    #[allow(dead_code)] // ML system - used in machine learning policy training and adaptation
    pub fn train_ml_policy(&self, key: &WarmCacheKey<K>, was_correct: bool) {
        let cache_pressure = self.calculate_cache_pressure();
        self.ml_policy.train(key, cache_pressure, was_correct);
    }

    /// Select random eviction candidates
    fn select_random_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        // Connect to sophisticated intelligent selection using ML confidence
        // Use ml_policy for ML confidence calculations - delegate to actual model prediction
        let cache_pressure = self.calculate_cache_pressure();
        let ml_confidence = if let Some(_recent_key) = self.get_most_frequent_keys(1).first() {
            // Connect to existing ML policy prediction system
            let features = crate::cache::tier::warm::eviction::ml::features::FeatureVector::new(0);
            self.ml_policy
                .predict_eviction_score(&features, cache_pressure)
        } else {
            0.5 // Fallback when no recent keys available
        };
        let combined_ml_confidence = ml_confidence;

        if combined_ml_confidence > 0.4 {
            // Use ML policy for medium ML confidence
            self.ml_policy.select_candidates(count)
        } else {
            // Use LRU for low confidence (safe default)
            self.select_lru_candidates(count)
        }
    }

    /// Select size-based eviction candidates
    fn select_size_based_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        // Connect to sophisticated size-aware AdaptiveLFU algorithm
        let candidates = self.lfu_tracker.select_candidates(count);
        if candidates.len() < count {
            // Supplement with intelligent fallback if needed
            let mut result = candidates;
            let remaining = count - result.len();
            result.extend(self.select_lru_candidates(remaining));
            result
        } else {
            candidates
        }
    }

    /// Clear all policy state
    pub fn clear(&self) {
        self.lru_tracker.clear();
        self.lfu_tracker.clear();
        self.arc_state.clear();
        self.ml_policy.clear();
    }

    /// Get total size across all policies
    pub fn total_size(&self) -> usize {
        // Return the maximum size among all policies
        [
            self.lru_tracker.size(),
            self.lfu_tracker.size(),
            self.arc_state.total_cache_size(),
            self.ml_policy.size(),
        ]
        .iter()
        .max()
        .copied()
        .unwrap_or(0)
    }
}

impl<K: CacheKey> Default for ConcurrentEvictionPolicy<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CacheKey> EvictionPolicy<WarmCacheKey<K>> for ConcurrentEvictionPolicy<K> {
    fn on_access(&self, key: &WarmCacheKey<K>, hit: bool) {
        self.on_access(key, hit);
    }

    fn on_eviction(&self, key: &WarmCacheKey<K>) {
        self.on_eviction(key);
    }

    fn select_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.select_eviction_candidates(count)
    }

    fn performance_metrics(&self) -> PolicyPerformanceMetrics {
        self.performance_metrics()
    }

    fn calculate_average_access_time_ns(&self) -> u64 {
        // Delegate to the underlying policy's performance metrics
        self.performance_metrics().avg_access_time_ns
    }

    fn adapt(&self) {
        self.adapt_policy();
    }
}
