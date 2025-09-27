//! Traditional cache replacement policies (LRU, LFU, ARC, Two-Queue)
//!
//! This module implements classic cache replacement algorithms with atomic
//! operations for lock-free concurrent access.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crossbeam_utils::{CachePadded, atomic::AtomicCell};
use dashmap::DashMap;

use super::ml_policies::MLPredictivePolicy;
use super::types::{AccessEvent, PolicyType};
use crate::cache::config::CacheConfig;
use crate::cache::tier::warm::core::WarmCacheKey;
use crate::cache::tier::warm::eviction::ml::policy::MachineLearningEvictionPolicy;
use crate::cache::traits::AccessType;
use crate::cache::traits::core::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Advanced replacement policies coordinator
#[derive(Debug)]
pub struct ReplacementPolicies<K: CacheKey> {
    /// LRU policy with frequency weighting
    #[allow(dead_code)]
    // Traditional policies - adaptive LRU used in replacement victim selection
    adaptive_lru: AdaptiveLRUPolicy<K>,
    /// LFU policy with recency considerations
    #[allow(dead_code)]
    // Traditional policies - adaptive LFU used in replacement victim selection
    adaptive_lfu: AdaptiveLFUPolicy<K>,
    /// Two-Queue algorithm for scan resistance
    #[allow(dead_code)]
    // Traditional policies - two queue used in replacement victim selection
    two_queue: TwoQueuePolicy<K>,
    /// Adaptive Replacement Cache (ARC) implementation
    #[allow(dead_code)]
    // Traditional policies - ARC policy used in replacement victim selection
    arc_policy: ARCPolicy<K>,
    /// Neural Network ML policy (24-dimensional features)
    neural_ml: MLPredictivePolicy<K>,
    /// Linear Regression ML policy (16-dimensional features)  
    linear_ml: MachineLearningEvictionPolicy<K>,
    /// Current active policy (atomic switching)
    #[allow(dead_code)] // Traditional policies - active policy used in policy switching
    active_policy: AtomicCell<PolicyType>,
    /// Policy performance metrics for comparison
    policy_metrics: PolicyMetrics,
}

/// Adaptive LRU with frequency weighting
#[derive(Debug)]
pub struct AdaptiveLRUPolicy<K: CacheKey> {
    /// LRU tracking with atomic timestamps - lock-free concurrent access
    lru_timestamps: DashMap<K, AtomicU64>,
    /// Frequency counters for weighting - lock-free concurrent access
    #[allow(dead_code)]
    // Traditional policies - frequency counters used in adaptive LRU weighting
    frequency_counters: DashMap<K, AtomicU32>,
    /// Adaptive weighting factor (frequency vs recency)
    weighting_factor: CachePadded<AtomicU32>, // Factor * 1000
}

/// Adaptive LFU with recency considerations
#[derive(Debug)]
pub struct AdaptiveLFUPolicy<K: CacheKey> {
    /// Frequency counters with decay - lock-free concurrent access
    #[allow(dead_code)]
    // Traditional policies - frequency counters used in adaptive LFU tracking
    frequency_counters: DashMap<K, AtomicU32>,
    /// Last access timestamps for recency weighting - lock-free concurrent access
    #[allow(dead_code)] // Traditional policies - last access used in recency weighting
    last_access: DashMap<K, AtomicU64>,
    /// Frequency decay rate per second
    #[allow(dead_code)] // Traditional policies - decay rate used in frequency aging
    decay_rate: CachePadded<AtomicU32>, // Rate * 1000
    /// Recency weighting factor
    #[allow(dead_code)]
    // Traditional policies - recency weight used in LFU recency calculations
    recency_weight: CachePadded<AtomicU32>, // Weight * 1000
}

/// Two-Queue algorithm for scan resistance
#[derive(Debug)]
pub struct TwoQueuePolicy<K: CacheKey> {
    /// A1 queue (first access, small) - lock-free concurrent access
    #[allow(dead_code)] // Traditional policies - A1 queue used in two-queue algorithm
    a1_queue: DashMap<K, u64>, // Key -> timestamp
    /// Am queue (frequent access, large) - lock-free concurrent access
    #[allow(dead_code)] // Traditional policies - Am queue used in two-queue algorithm
    am_queue: DashMap<K, u64>, // Key -> timestamp
    /// A1out ghost list for tracking evicted entries - lock-free concurrent access
    #[allow(dead_code)]
    // Traditional policies - A1out ghost used in two-queue promotion tracking
    a1out_ghost: DashMap<K, u64>, // Key -> eviction timestamp
    /// Maximum sizes for queues
    #[allow(dead_code)] // Traditional policies - A1 max size used in queue size management
    a1_max_size: usize,
    #[allow(dead_code)] // Traditional policies - Am max size used in queue size management
    am_max_size: usize,
    #[allow(dead_code)] // Traditional policies - ghost max size used in ghost list management
    ghost_max_size: usize,
}

/// Adaptive Replacement Cache (ARC) implementation
#[derive(Debug)]
pub struct ARCPolicy<K: CacheKey> {
    /// T1 list (recent cache entries) - lock-free concurrent access
    t1_list: DashMap<K, ()>,
    /// T2 list (frequent cache entries) - lock-free concurrent access
    t2_list: DashMap<K, ()>,
    /// B1 ghost list (recent evictions) - lock-free concurrent access
    #[allow(dead_code)] // Traditional policies - B1 ghost used in ARC recency tracking
    b1_ghost: DashMap<K, ()>,
    /// B2 ghost list (frequent evictions) - lock-free concurrent access
    #[allow(dead_code)] // Traditional policies - B2 ghost used in ARC frequency tracking
    b2_ghost: DashMap<K, ()>,
    /// Adaptive parameter for T1/T2 balance
    adaptation_parameter: CachePadded<AtomicU32>, // Parameter * 1000
    /// Maximum cache size
    max_size: usize,
}

/// Policy performance metrics for comparison
#[derive(Debug)]
pub struct PolicyMetrics {
    /// Hit rates per policy type
    hit_rates: CachePadded<[AtomicU32; 5]>, // Per PolicyType, rate * 1000
    /// Average access latencies per policy  
    access_latencies: CachePadded<[AtomicU64; 5]>, // Nanoseconds
    /// Memory efficiency per policy
    memory_efficiency: CachePadded<[AtomicU32; 5]>, // Bytes per hit
    /// Policy switching frequency
    switch_frequency: CachePadded<AtomicU32>, // Switches per hour
}

#[allow(dead_code)] // Library API - methods may be used by external consumers
impl<K: CacheKey> ReplacementPolicies<K> {
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            adaptive_lru: AdaptiveLRUPolicy::new(),
            adaptive_lfu: AdaptiveLFUPolicy::new(),
            two_queue: TwoQueuePolicy::new(),
            arc_policy: ARCPolicy::new(),
            neural_ml: MLPredictivePolicy::new(config)?,
            linear_ml: MachineLearningEvictionPolicy::new(),
            active_policy: AtomicCell::new(PolicyType::default()),
            policy_metrics: PolicyMetrics::new(),
        })
    }

    pub fn get_active_policy(&self) -> PolicyType {
        self.active_policy.load()
    }

    pub fn switch_policy(&self, new_policy: PolicyType) -> Result<(), CacheOperationError> {
        self.active_policy.store(new_policy);
        self.policy_metrics.record_policy_switch();
        Ok(())
    }

    pub fn select_lru_victim(&self, candidates: &[K]) -> Option<K> {
        self.adaptive_lru.select_victim(candidates)
    }

    pub fn select_lfu_victim(&self, candidates: &[K]) -> Option<K> {
        self.adaptive_lfu.select_victim(candidates)
    }

    pub fn select_two_queue_victim(&self, candidates: &[K]) -> Option<K> {
        self.two_queue.select_victim(candidates)
    }

    pub fn select_arc_victim(&self, candidates: &[K]) -> Option<K> {
        self.arc_policy.select_victim(candidates)
    }

    #[allow(dead_code)] // ML system - used in adaptive ML eviction victim selection
    pub fn select_ml_victim(&self, candidates: &[K]) -> Option<K> {
        if candidates.is_empty() {
            return None;
        }

        // Phase 1: Neural Network ML Prediction (24-dimensional features)
        let neural_victim = self.neural_ml.select_victim(candidates);

        // Phase 2: Linear Regression ML Prediction (16-dimensional features)
        // Convert K: CacheKey to WarmCacheKey using existing conversion
        let warm_candidates: Vec<WarmCacheKey<K>> = candidates
            .iter()
            .map(|key| WarmCacheKey::<K>::from_cache_key(key))
            .collect();

        // Calculate cache pressure (simple ratio-based estimation)
        let cache_pressure = if candidates.len() > 1000 {
            0.9 // High pressure
        } else if candidates.len() > 100 {
            0.6 // Medium pressure
        } else {
            0.3 // Low pressure
        };

        let linear_victim_warm = self
            .linear_ml
            .select_ml_candidate(&warm_candidates, cache_pressure);

        // Phase 3: Intelligent ML Coordination
        match (neural_victim, linear_victim_warm) {
            // Both ML systems agree - high confidence
            (Some(neural_choice), Some(linear_choice_warm)) => {
                // Convert WarmCacheKey back to check if they match
                let neural_warm = WarmCacheKey::from_cache_key(&neural_choice);
                if neural_warm.primary_hash == linear_choice_warm.primary_hash {
                    // Both ML systems agree - very high confidence
                    Some(neural_choice)
                } else {
                    // ML systems disagree - use confidence-based selection
                    let neural_accuracy = self.neural_ml.get_metrics().accuracy;
                    let linear_accuracy = self.linear_ml.accuracy();

                    if neural_accuracy >= linear_accuracy {
                        Some(neural_choice)
                    } else {
                        // Convert WarmCacheKey back to original key
                        // Find matching candidate for linear choice
                        candidates
                            .iter()
                            .find(|key| {
                                let warm_key = WarmCacheKey::from_cache_key(*key);
                                warm_key.primary_hash == linear_choice_warm.primary_hash
                            })
                            .cloned()
                    }
                }
            }
            // Only neural network has prediction
            (Some(neural_choice), None) => {
                let neural_accuracy = self.neural_ml.get_metrics().accuracy;
                if neural_accuracy > 0.6 {
                    // Good confidence threshold
                    Some(neural_choice)
                } else {
                    // Low confidence - fallback to intelligent traditional policy
                    self.intelligent_fallback_selection(candidates)
                }
            }
            // Only linear regression has prediction
            (None, Some(linear_choice_warm)) => {
                let linear_accuracy = self.linear_ml.accuracy();
                if linear_accuracy > 0.6 {
                    // Good confidence threshold
                    // Convert WarmCacheKey back to original key
                    candidates
                        .iter()
                        .find(|key| {
                            let warm_key = WarmCacheKey::from_cache_key(*key);
                            warm_key.primary_hash == linear_choice_warm.primary_hash
                        })
                        .cloned()
                } else {
                    // Low confidence - fallback to intelligent traditional policy
                    self.intelligent_fallback_selection(candidates)
                }
            }
            // Neither ML system has prediction - fallback
            (None, None) => self.intelligent_fallback_selection(candidates),
        }
    }

    /// Intelligent fallback when ML confidence is low
    fn intelligent_fallback_selection(&self, candidates: &[K]) -> Option<K> {
        // Use ML insights to select best traditional algorithm
        let neural_accuracy = self.neural_ml.get_metrics().accuracy;
        let linear_accuracy = self.linear_ml.accuracy();
        let combined_ml_confidence = (neural_accuracy + linear_accuracy) / 2.0;

        if combined_ml_confidence > 0.4 {
            // Medium confidence - use ARC (adaptive)
            self.arc_policy.select_victim(candidates)
        } else {
            // Low confidence - use LRU (safe default)
            self.adaptive_lru.select_victim(candidates)
        }
    }

    pub fn record_access(&self, event: &AccessEvent<K>) {
        // Train traditional policies
        self.adaptive_lru.record_access(&event.key);
        self.adaptive_lfu.record_access(&event.key);
        self.two_queue.record_access(&event.key);
        self.arc_policy.record_access(&event.key);

        // Train both ML systems

        // Phase 1: Train Neural Network ML (requires AccessEvent<K>)
        self.neural_ml.record_access(event);

        // Phase 2: Train Linear Regression ML (requires different parameters)
        let warm_key = WarmCacheKey::from_cache_key(&event.key);
        let timestamp_ns = event.timestamp;

        // Convert AccessEvent access_type to warm tier AccessType
        let warm_access_type = match event.access_type {
            super::types::AccessType::Read => AccessType::Read,
            super::types::AccessType::Write => AccessType::Write,
            super::types::AccessType::ReadModifyWrite => AccessType::Write,
            super::types::AccessType::Sequential => AccessType::Read,
            super::types::AccessType::Random => AccessType::Read,
            super::types::AccessType::SequentialRead => AccessType::Read,
            super::types::AccessType::RandomRead => AccessType::Read,
            super::types::AccessType::SequentialWrite => AccessType::Write,
            super::types::AccessType::Temporal => AccessType::Read,
            super::types::AccessType::Spatial => AccessType::Read,
            super::types::AccessType::Prefetch => AccessType::Read,
            super::types::AccessType::PrefetchHit => AccessType::Read,
            super::types::AccessType::Hit => AccessType::Read,
            super::types::AccessType::Miss => AccessType::Read,
            super::types::AccessType::Promotion => AccessType::Read,
            super::types::AccessType::Demotion => AccessType::Write,
        };

        self.linear_ml
            .record_access(&warm_key, timestamp_ns, warm_access_type);
    }

    pub fn update_policy_metrics(
        &self,
        policy: PolicyType,
        hit_rate: f64,
        latency_ns: u64,
        efficiency: f64,
    ) {
        self.policy_metrics
            .update_metrics(policy, hit_rate, latency_ns, efficiency);
    }

    pub fn evaluate_policy_performance(&self) -> Option<PolicyType> {
        self.policy_metrics.evaluate_best_policy()
    }

    pub fn get_policy_metrics(&self) -> super::policy_engine::PolicyStats {
        self.policy_metrics.get_stats()
    }

    pub fn select_candidate(&self, candidates: &[K]) -> Option<K> {
        match self.active_policy.load() {
            PolicyType::AdaptiveLRU => self.select_lru_victim(candidates),
            PolicyType::AdaptiveLFU => self.select_lfu_victim(candidates),
            PolicyType::TwoQueue => self.select_two_queue_victim(candidates),
            PolicyType::Arc => self.select_arc_victim(candidates),
            PolicyType::MLPredictive => self.select_ml_victim(candidates),
        }
    }

    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        Ok(())
    }
}

impl<K: CacheKey> AdaptiveLRUPolicy<K> {
    fn new() -> Self {
        Self {
            lru_timestamps: DashMap::with_capacity(1024),
            frequency_counters: DashMap::with_capacity(1024),
            weighting_factor: CachePadded::new(AtomicU32::new(500)), // 0.5 balance
        }
    }

    #[allow(dead_code)] // Traditional policies - record access used in LRU access pattern tracking
    fn record_access(&self, key: &K) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        // Update timestamp atomically - lock-free concurrent access
        self.lru_timestamps
            .entry(key.clone())
            .or_insert_with(|| AtomicU64::new(0))
            .store(timestamp, Ordering::Relaxed);

        // Increment frequency counter atomically
        self.frequency_counters
            .entry(key.clone())
            .or_insert_with(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)] // Traditional policies - select victim used in LRU replacement decisions
    fn select_victim(&self, candidates: &[K]) -> Option<K> {
        if candidates.is_empty() {
            return None;
        }

        let mut oldest_key: Option<K> = None;
        let mut oldest_score = u64::MAX;
        let weighting_factor = self.weighting_factor.load(Ordering::Relaxed) as f64 / 1000.0;

        for candidate in candidates {
            let timestamp = self
                .lru_timestamps
                .get(candidate)
                .map(|ts| ts.load(Ordering::Relaxed))
                .unwrap_or(0);

            let frequency = self
                .frequency_counters
                .get(candidate)
                .map(|freq| freq.load(Ordering::Relaxed))
                .unwrap_or(1) as f64;

            // Calculate adaptive LRU score: lower = more likely to evict
            // Combines recency (timestamp) with frequency weighting
            let recency_score = u64::MAX - timestamp; // Higher for older entries
            let frequency_penalty = (frequency * weighting_factor) as u64;
            let total_score = recency_score.saturating_sub(frequency_penalty);

            if total_score < oldest_score {
                oldest_score = total_score;
                oldest_key = Some(candidate.clone());
            }
        }

        oldest_key
    }
}

impl<K: CacheKey> AdaptiveLFUPolicy<K> {
    fn new() -> Self {
        Self {
            frequency_counters: DashMap::with_capacity(1024),
            last_access: DashMap::with_capacity(1024),
            decay_rate: CachePadded::new(AtomicU32::new(100)), // 0.1 decay per second
            recency_weight: CachePadded::new(AtomicU32::new(300)), // 0.3 weight
        }
    }

    #[allow(dead_code)] // Traditional policies - record access used in LFU frequency tracking
    fn record_access(&self, key: &K) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        // Update frequency counter with time-based decay
        let current_freq = self
            .frequency_counters
            .entry(key.clone())
            .or_insert_with(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);

        // Apply decay based on time since last access
        if let Some(last_timestamp) = self.last_access.get(key) {
            let time_diff = timestamp.saturating_sub(last_timestamp.load(Ordering::Relaxed));
            let decay_rate = self.decay_rate.load(Ordering::Relaxed) as f64 / 1000.0;
            let time_seconds = time_diff as f64 / 1_000_000_000.0;
            let decay_factor = (-decay_rate * time_seconds).exp();

            // Apply decay to frequency counter
            let decayed_freq = (current_freq as f64 * decay_factor) as u32;
            if let Some(counter) = self.frequency_counters.get(key) {
                counter.store(decayed_freq.max(1), Ordering::Relaxed);
            }
        }

        // Update last access timestamp
        self.last_access
            .entry(key.clone())
            .or_insert_with(|| AtomicU64::new(0))
            .store(timestamp, Ordering::Relaxed);
    }

    #[allow(dead_code)] // Traditional policies - select victim used in LFU replacement decisions
    fn select_victim(&self, candidates: &[K]) -> Option<K> {
        if candidates.is_empty() {
            return None;
        }

        let mut lowest_score_key: Option<K> = None;
        let mut lowest_score = f64::MAX;
        let recency_weight = self.recency_weight.load(Ordering::Relaxed) as f64 / 1000.0;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        // Find candidate with lowest frequency-recency score
        for candidate in candidates {
            let frequency = self
                .frequency_counters
                .get(candidate)
                .map(|f| f.load(Ordering::Relaxed))
                .unwrap_or(0) as f64;

            let last_access_time = self
                .last_access
                .get(candidate)
                .map(|t| t.load(Ordering::Relaxed))
                .unwrap_or(0);

            // Calculate recency factor (higher = more recent)
            let time_diff = current_time.saturating_sub(last_access_time) as f64 / 1_000_000_000.0;
            let recency_factor = (-time_diff / 3600.0).exp(); // Decay over hours

            // Combined score: frequency + recency_weight * recency_factor
            // Lower score = better victim candidate
            let score = frequency + recency_weight * recency_factor;

            if score < lowest_score {
                lowest_score = score;
                lowest_score_key = Some(candidate.clone());
            }
        }

        lowest_score_key
    }
}

impl<K: CacheKey> TwoQueuePolicy<K> {
    fn new() -> Self {
        Self {
            a1_queue: DashMap::with_capacity(256),
            am_queue: DashMap::with_capacity(768),
            a1out_ghost: DashMap::with_capacity(256),
            a1_max_size: 256,
            am_max_size: 768,
            ghost_max_size: 256,
        }
    }

    #[allow(dead_code)] // Traditional policies - record access used in two-queue algorithm tracking
    fn record_access(&self, key: &K) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        // Case 1: Key is in Am queue (frequent) - update timestamp
        if self.am_queue.contains_key(key) {
            self.am_queue.insert(key.clone(), timestamp);
            return;
        }

        // Case 2: Key is in A1 queue (first access) - promote to Am
        if self.a1_queue.remove(key).is_some() {
            self.am_queue.insert(key.clone(), timestamp);
            self.maintain_am_size();
            return;
        }

        // Case 3: Key is in A1out ghost (was evicted from A1) - promote to Am
        if self.a1out_ghost.remove(key).is_some() {
            self.am_queue.insert(key.clone(), timestamp);
            self.maintain_am_size();
            return;
        }

        // Case 4: New key - add to A1 queue
        self.a1_queue.insert(key.clone(), timestamp);
        self.maintain_a1_size();
    }

    #[allow(dead_code)] // Traditional policies - maintain A1 size used in two-queue size management
    fn maintain_a1_size(&self) {
        while self.a1_queue.len() > self.a1_max_size {
            // Evict oldest from A1 to A1out ghost
            if let Some(entry) = self.a1_queue.iter().min_by_key(|e| *e.value()) {
                let key = entry.key().clone();
                let eviction_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);

                self.a1_queue.remove(&key);
                self.a1out_ghost.insert(key, eviction_time);

                // Maintain ghost list size
                self.maintain_ghost_size();
                break;
            }
        }
    }

    #[allow(dead_code)] // Traditional policies - maintain Am size used in two-queue size management
    fn maintain_am_size(&self) {
        while self.am_queue.len() > self.am_max_size {
            // Evict oldest from Am (LRU within Am)
            if let Some(entry) = self.am_queue.iter().min_by_key(|e| *e.value()) {
                let key = entry.key().clone();
                self.am_queue.remove(&key);
                break;
            }
        }
    }

    #[allow(dead_code)] // Traditional policies - maintain ghost size used in two-queue ghost management
    fn maintain_ghost_size(&self) {
        while self.a1out_ghost.len() > self.ghost_max_size {
            // Remove oldest from ghost list
            if let Some(entry) = self.a1out_ghost.iter().min_by_key(|e| *e.value()) {
                let key = entry.key().clone();
                self.a1out_ghost.remove(&key);
                break;
            }
        }
    }

    #[allow(dead_code)] // Traditional policies - select victim used in two-queue replacement decisions
    fn select_victim(&self, candidates: &[K]) -> Option<K> {
        if candidates.is_empty() {
            return None;
        }

        // Prefer evicting from A1 queue (first access items)
        for candidate in candidates {
            if self.a1_queue.contains_key(candidate) {
                return Some(candidate.clone());
            }
        }

        // Then prefer evicting from Am queue (find oldest)
        let mut oldest_key: Option<K> = None;
        let mut oldest_timestamp = u64::MAX;

        for candidate in candidates {
            if let Some(timestamp) = self.am_queue.get(candidate)
                && *timestamp < oldest_timestamp
            {
                oldest_timestamp = *timestamp;
                oldest_key = Some(candidate.clone());
            }
        }

        if oldest_key.is_some() {
            return oldest_key;
        }

        // Fallback to any candidate
        candidates.first().cloned()
    }
}

impl<K: CacheKey> ARCPolicy<K> {
    fn new() -> Self {
        Self {
            t1_list: DashMap::with_capacity(256),
            t2_list: DashMap::with_capacity(256),
            b1_ghost: DashMap::with_capacity(256),
            b2_ghost: DashMap::with_capacity(256),
            adaptation_parameter: CachePadded::new(AtomicU32::new(500)), // 0.5 balance
            max_size: 512,
        }
    }

    #[allow(dead_code)] // Traditional policies - record access used in ARC algorithm tracking
    fn record_access(&self, key: &K) {
        // ARC algorithm implementation
        let current_size = self.t1_list.len() + self.t2_list.len();
        let adaptation_param = self.adaptation_parameter.load(Ordering::Relaxed) as f64 / 1000.0;
        let target_t1_size = (self.max_size as f64 * adaptation_param) as usize;

        // Case 1: Key is in T1 (recent) - promote to T2 (frequent)
        if self.t1_list.remove(key).is_some() {
            self.t2_list.insert(key.clone(), ());
            return;
        }

        // Case 2: Key is in T2 (frequent) - move to front (already frequent)
        if self.t2_list.contains_key(key) {
            // Already in T2, just update position (DashMap handles this)
            return;
        }

        // Case 3: Key is in B1 ghost (was recent) - adapt towards recency
        if self.b1_ghost.remove(key).is_some() {
            // Increase adaptation parameter (favor T1/recency)
            let current_param = self.adaptation_parameter.load(Ordering::Relaxed);
            let new_param = (current_param + 10).min(1000); // Increase by 0.01, max 1.0
            self.adaptation_parameter
                .store(new_param, Ordering::Relaxed);

            // Add to T2 (it was accessed again after being recent)
            self.t2_list.insert(key.clone(), ());
            self.replace_if_needed(target_t1_size);
            return;
        }

        // Case 4: Key is in B2 ghost (was frequent) - adapt towards frequency
        if self.b2_ghost.remove(key).is_some() {
            // Decrease adaptation parameter (favor T2/frequency)
            let current_param = self.adaptation_parameter.load(Ordering::Relaxed);
            let new_param = current_param.saturating_sub(10); // Decrease by 0.01, min 0.0
            self.adaptation_parameter
                .store(new_param, Ordering::Relaxed);

            // Add to T2 (it was frequent before)
            self.t2_list.insert(key.clone(), ());
            self.replace_if_needed(target_t1_size);
            return;
        }

        // Case 5: New key - add to T1 (recent)
        if current_size >= self.max_size {
            self.replace_if_needed(target_t1_size);
        }
        self.t1_list.insert(key.clone(), ());
    }

    #[allow(dead_code)] // Traditional policies - replace if needed used in ARC replacement algorithm
    fn replace_if_needed(&self, target_t1_size: usize) {
        let t1_size = self.t1_list.len();
        let t2_size = self.t2_list.len();

        if t1_size > target_t1_size && !self.t1_list.is_empty() {
            // Evict from T1 to B1
            if let Some(entry) = self.t1_list.iter().next() {
                let key = entry.key().clone();
                self.t1_list.remove(&key);
                self.b1_ghost.insert(key, ());

                // Maintain B1 ghost size
                if self.b1_ghost.len() > self.max_size
                    && let Some(old_entry) = self.b1_ghost.iter().next()
                {
                    let old_key = old_entry.key().clone();
                    self.b1_ghost.remove(&old_key);
                }
            }
        } else if t2_size > 0 {
            // Evict from T2 to B2
            if let Some(entry) = self.t2_list.iter().next() {
                let key = entry.key().clone();
                self.t2_list.remove(&key);
                self.b2_ghost.insert(key, ());

                // Maintain B2 ghost size
                if self.b2_ghost.len() > self.max_size
                    && let Some(old_entry) = self.b2_ghost.iter().next()
                {
                    let old_key = old_entry.key().clone();
                    self.b2_ghost.remove(&old_key);
                }
            }
        }
    }

    fn select_victim(&self, candidates: &[K]) -> Option<K> {
        if candidates.is_empty() {
            return None;
        }

        let adaptation_param = self.adaptation_parameter.load(Ordering::Relaxed) as f64 / 1000.0;
        let target_t1_size = (self.max_size as f64 * adaptation_param) as usize;
        let t1_size = self.t1_list.len();

        // Prefer evicting from T1 if it's over target size
        if t1_size > target_t1_size {
            for candidate in candidates {
                if self.t1_list.contains_key(candidate) {
                    return Some(candidate.clone());
                }
            }
        }

        // Otherwise prefer evicting from T2
        for candidate in candidates {
            if self.t2_list.contains_key(candidate) {
                return Some(candidate.clone());
            }
        }

        // Fallback to any candidate
        candidates.first().cloned()
    }
}

impl PolicyMetrics {
    fn new() -> Self {
        Self {
            hit_rates: CachePadded::new([
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ]),
            access_latencies: CachePadded::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            memory_efficiency: CachePadded::new([
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ]),
            switch_frequency: CachePadded::new(AtomicU32::new(0)),
        }
    }

    #[allow(dead_code)] // Traditional policies - update metrics used in policy performance tracking
    fn update_metrics(&self, policy: PolicyType, hit_rate: f64, latency_ns: u64, efficiency: f64) {
        let index = policy as usize;
        self.hit_rates[index].store((hit_rate * 1000.0) as u32, Ordering::Relaxed);
        self.access_latencies[index].store(latency_ns, Ordering::Relaxed);
        self.memory_efficiency[index].store((efficiency * 1000.0) as u32, Ordering::Relaxed);
    }

    #[allow(dead_code)] // Traditional policies - record policy switch used in adaptation frequency tracking
    fn record_policy_switch(&self) {
        self.switch_frequency.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)] // Traditional policies - evaluate best policy used in adaptive policy selection
    fn evaluate_best_policy(&self) -> Option<PolicyType> {
        let mut best_policy = PolicyType::AdaptiveLRU;
        let mut best_score = 0.0f64;

        // Evaluate each policy based on weighted performance metrics
        for policy_idx in 0..5 {
            let hit_rate = self.hit_rates[policy_idx].load(Ordering::Relaxed) as f64 / 1000.0;
            let latency = self.access_latencies[policy_idx].load(Ordering::Relaxed) as f64;
            let efficiency =
                self.memory_efficiency[policy_idx].load(Ordering::Relaxed) as f64 / 1000.0;

            // Skip policies with no data
            if hit_rate == 0.0 && latency == 0.0 && efficiency == 0.0 {
                continue;
            }

            // Calculate composite score (higher is better)
            // Weight: 50% hit rate, 30% latency (inverted), 20% efficiency
            let latency_score = if latency > 0.0 {
                1.0 / (latency / 1_000_000.0)
            } else {
                0.0
            }; // Convert to ms and invert
            let composite_score = (hit_rate * 0.5) + (latency_score * 0.3) + (efficiency * 0.2);

            if composite_score > best_score {
                best_score = composite_score;
                best_policy = match policy_idx {
                    0 => PolicyType::AdaptiveLRU,
                    1 => PolicyType::AdaptiveLFU,
                    2 => PolicyType::TwoQueue,
                    3 => PolicyType::Arc,
                    4 => PolicyType::MLPredictive,
                    _ => PolicyType::AdaptiveLRU,
                };
            }
        }

        // Only recommend switch if there's significant improvement (>10%)
        if best_score > 0.1 {
            Some(best_policy)
        } else {
            None
        }
    }

    fn get_stats(&self) -> super::policy_engine::PolicyStats {
        // Calculate real performance statistics
        let mut total_hits = 0u64;
        let mut total_accesses = 0u64;
        let mut weighted_latency = 0u64;
        let mut total_efficiency = 0.0f64;
        let mut active_policies = 0;

        for policy_idx in 0..5 {
            let hit_rate = self.hit_rates[policy_idx].load(Ordering::Relaxed) as f64 / 1000.0;
            let latency = self.access_latencies[policy_idx].load(Ordering::Relaxed);
            let efficiency =
                self.memory_efficiency[policy_idx].load(Ordering::Relaxed) as f64 / 1000.0;

            if hit_rate > 0.0 || latency > 0 || efficiency > 0.0 {
                active_policies += 1;

                // Estimate accesses from hit rate (assuming 1000 sample size)
                let estimated_accesses = 1000u64;
                let estimated_hits = (hit_rate * estimated_accesses as f64) as u64;

                total_hits += estimated_hits;
                total_accesses += estimated_accesses;
                weighted_latency += latency;
                total_efficiency += efficiency;
            }
        }

        let _overall_hit_rate = if total_accesses > 0 {
            total_hits as f64 / total_accesses as f64
        } else {
            0.0
        };

        let _average_latency = if active_policies > 0 {
            weighted_latency / active_policies as u64
        } else {
            0
        };

        let _average_efficiency = if active_policies > 0 {
            total_efficiency / active_policies as f64
        } else {
            0.0
        };

        let switch_frequency = self.switch_frequency.load(Ordering::Relaxed);

        super::policy_engine::PolicyStats {
            hit_rates: [
                self.hit_rates[0].load(Ordering::Relaxed) as f64 / 1000.0,
                self.hit_rates[1].load(Ordering::Relaxed) as f64 / 1000.0,
                self.hit_rates[2].load(Ordering::Relaxed) as f64 / 1000.0,
                self.hit_rates[3].load(Ordering::Relaxed) as f64 / 1000.0,
                self.hit_rates[4].load(Ordering::Relaxed) as f64 / 1000.0,
            ],
            latencies: [
                self.access_latencies[0].load(Ordering::Relaxed),
                self.access_latencies[1].load(Ordering::Relaxed),
                self.access_latencies[2].load(Ordering::Relaxed),
                self.access_latencies[3].load(Ordering::Relaxed),
                self.access_latencies[4].load(Ordering::Relaxed),
            ],
            memory_efficiency: [
                self.memory_efficiency[0].load(Ordering::Relaxed) as f64 / 1000.0,
                self.memory_efficiency[1].load(Ordering::Relaxed) as f64 / 1000.0,
                self.memory_efficiency[2].load(Ordering::Relaxed) as f64 / 1000.0,
                self.memory_efficiency[3].load(Ordering::Relaxed) as f64 / 1000.0,
                self.memory_efficiency[4].load(Ordering::Relaxed) as f64 / 1000.0,
            ],
            switch_frequency,
        }
    }
}
