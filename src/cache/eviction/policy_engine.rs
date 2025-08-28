//! Cache policy engine with adaptive switching and pattern analysis
//!
//! This module implements the core policy engine that coordinates different
//! eviction strategies and adapts based on workload characteristics.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use crate::cache::analyzer::types::AccessPattern;
use crate::cache::analyzer::analyzer_core::AccessPatternAnalyzer;
use crate::cache::coherence::CacheTier;
use crate::cache::config::CacheConfig;
use crate::cache::manager::policy::types::WritePolicy;
use super::prefetch::PrefetchPredictor;
use super::traditional_policies::ReplacementPolicies;
use super::types::{AccessEvent, PolicyType};
use super::write_policies::WritePolicyManager;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Cache policy engine with machine learning-based decisions
#[derive(Debug)]
pub struct CachePolicyEngine<K: CacheKey, V: CacheValue> {
    /// Access pattern analyzer with ML prediction
    pub pattern_analyzer: AccessPatternAnalyzer<K>,
    /// Advanced replacement policies with adaptive algorithms
    replacement_policies: ReplacementPolicies<K>,
    /// Write policy manager with consistency guarantees
    write_policy_manager: WritePolicyManager<K>,
    /// Prefetch predictor with pattern recognition
    prefetch_predictor: PrefetchPredictor<K>,
    /// Current active policy type (atomic for lock-free switching)
    current_policy: AtomicU8,
    /// Phantom data to maintain type parameter
    pub _phantom: PhantomData<V>,
}

impl<K: CacheKey, V: CacheValue> CachePolicyEngine<K, V> {
    /// Create new cache policy engine with configuration and policy type
    pub fn new(
        config: &CacheConfig,
        initial_policy: PolicyType,
    ) -> Result<Self, CacheOperationError> {
        let pattern_analyzer =
            AccessPatternAnalyzer::new(config.analyzer.clone()).map_err(|e| {
                CacheOperationError::initialization_failed(&format!(
                    "Failed to initialize pattern analyzer: {:?}",
                    e
                ))
            })?;

        Ok(Self {
            pattern_analyzer,
            replacement_policies: ReplacementPolicies::new(config)?,
            write_policy_manager: WritePolicyManager::new(config)?,
            prefetch_predictor: PrefetchPredictor::new(config)?,
            current_policy: AtomicU8::new(Self::policy_to_u8(initial_policy)),
            _phantom: PhantomData,
        })
    }

    /// Convert PolicyType to u8 for atomic storage
    #[inline]
    fn policy_to_u8(policy: PolicyType) -> u8 {
        match policy {
            PolicyType::AdaptiveLRU => 0,
            PolicyType::AdaptiveLFU => 1,
            PolicyType::TwoQueue => 2,
            PolicyType::ARC => 3,
            PolicyType::MLPredictive => 4,
        }
    }

    /// Convert u8 to PolicyType for atomic loading
    #[inline]
    fn u8_to_policy(value: u8) -> PolicyType {
        match value {
            0 => PolicyType::AdaptiveLRU,
            1 => PolicyType::AdaptiveLFU,
            2 => PolicyType::TwoQueue,
            3 => PolicyType::ARC,
            4 => PolicyType::MLPredictive,
            _ => PolicyType::AdaptiveLRU, // Default fallback
        }
    }

    /// Get current active policy type
    #[inline]
    pub fn current_policy(&self) -> PolicyType {
        Self::u8_to_policy(self.current_policy.load(Ordering::Relaxed))
    }

    /// Analyze access pattern for intelligent caching decisions
    pub fn analyze_access_pattern(&self, key: &K) -> AccessPattern {
        // Delegate to the pattern analyzer for consistent analysis
        self.pattern_analyzer.analyze_access_pattern(key)
    }

    /// Select replacement candidate using current sophisticated policy
    pub fn select_replacement_candidate(&self, _tier: CacheTier, candidates: &[K]) -> Option<K> {
        if candidates.is_empty() {
            return None;
        }

        // Dispatch to appropriate sophisticated policy algorithm
        let policy = self.current_policy();
        match policy {
            PolicyType::AdaptiveLRU => self.replacement_policies.select_lru_victim(candidates),
            PolicyType::AdaptiveLFU => self.replacement_policies.select_lfu_victim(candidates),
            PolicyType::TwoQueue => self
                .replacement_policies
                .select_two_queue_victim(candidates),
            PolicyType::ARC => self.replacement_policies.select_arc_victim(candidates),
            PolicyType::MLPredictive => self.replacement_policies.select_ml_victim(candidates),
        }
    }

    /// Generate prefetch predictions based on access patterns
    pub fn generate_prefetch_predictions(
        &self,
        current_key: &K,
        access_history: &[AccessEvent<K>],
    ) -> Vec<super::types::PrefetchRequest<K>> {
        let pattern = self.pattern_analyzer.analyze_access_pattern(current_key);

        // Only generate predictions for keys with good access patterns
        if pattern.frequency < 1.0 || pattern.recency < 0.3 {
            return Vec::new();
        }

        // Analyze access history for sequential patterns
        if access_history.len() >= 2 && pattern.temporal_locality > 0.6 {
            // Record the recent access events to build pattern history
            for event in access_history.iter().take(10) {
                self.prefetch_predictor.record_access(event);
            }
            
            // Use the existing PrefetchPredictor to generate predictions
            // The predictor has its own pattern detection and ML-based prediction
            let predictions = self.prefetch_predictor.get_next_prefetches(5);
            
            // Convert hot tier predictions to eviction tier format
            predictions.into_iter()
                .map(|p| super::types::PrefetchRequest {
                    key: p.key,
                    predicted_access: p.predicted_access_time,
                    priority: 5, // Default medium priority
                    confidence: match p.confidence {
                        crate::cache::tier::hot::prefetch::PredictionConfidence::Low => 0.25,
                        crate::cache::tier::hot::prefetch::PredictionConfidence::Medium => 0.50,
                        crate::cache::tier::hot::prefetch::PredictionConfidence::High => 0.75,
                        crate::cache::tier::hot::prefetch::PredictionConfidence::VeryHigh => 0.95,
                    },
                    created_at: std::time::Instant::now(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Update policy performance metrics and adapt if necessary
    pub fn update_performance_metrics(
        &self,
        policy: PolicyType,
        hit_rate: f64,
        latency_ns: u64,
        memory_efficiency: f64,
    ) {
        self.replacement_policies.update_policy_metrics(
            policy,
            hit_rate,
            latency_ns,
            memory_efficiency,
        );
    }

    /// Check if policy adaptation is needed based on performance
    pub fn should_adapt_policy(&self) -> Option<PolicyType> {
        self.replacement_policies.evaluate_policy_performance()
    }

    /// Adapt to new policy based on performance analysis
    pub fn adapt_policy(&self, new_policy: PolicyType) -> Result<(), CacheOperationError> {
        // Atomically switch to new policy
        self.current_policy
            .store(Self::policy_to_u8(new_policy), Ordering::Relaxed);
        self.replacement_policies.switch_policy(new_policy)
    }

    /// Record access event for pattern analysis
    pub fn record_access(&self, event: AccessEvent<K>) {
        let _ = self.pattern_analyzer.record_access(&event.key);
        self.replacement_policies.record_access(&event);
        self.prefetch_predictor.record_access(&event);
    }

    /// Get current policy performance statistics
    pub fn get_policy_stats(&self) -> PolicyStats {
        self.replacement_policies.get_policy_metrics()
    }

    /// Process write operation with configured policy
    pub fn process_write_operation(
        &self,
        key: &K,
        tier: CacheTier,
    ) -> Result<WriteResult, CacheOperationError> {
        // Analyze access pattern to determine optimal write strategy
        let pattern = self.pattern_analyzer.analyze_access_pattern(key);

        // Implement write policy logic with proper types
        let _write_policy = match tier {
            CacheTier::Hot => {
                if pattern.frequency > 5.0 && pattern.recency > 0.8 {
                    WritePolicy::WriteBack // High frequency, recent access
                } else {
                    WritePolicy::WriteThrough
                }
            }
            CacheTier::Warm => {
                if pattern.temporal_locality > 0.7 {
                    WritePolicy::WriteBack // Good temporal locality
                } else {
                    WritePolicy::WriteThrough
                }
            }
            CacheTier::Cold => WritePolicy::WriteThrough, // Always write through for durability
        };

        // Measure actual write operation latency
        let start_time = Instant::now();
        let result = self.write_policy_manager.process_write(key, tier);
        let actual_latency = start_time.elapsed().as_nanos() as u64;

        // Return write result with real timing
        match result {
            Ok(write_result) => Ok(WriteResult {
                success: write_result.success,
                latency_ns: actual_latency,
                tier,
            }),
            Err(e) => Err(e),
        }
    }

    /// Get write policy statistics
    pub fn get_write_stats(&self) -> WriteStats {
        self.write_policy_manager.get_statistics()
    }

    /// Execute prefetch operations
    pub fn execute_prefetch(
        &self,
        requests: &[super::types::PrefetchRequest<K>],
    ) -> PrefetchResult {
        self.prefetch_predictor.execute_prefetch(requests)
    }

    /// Get prefetch performance metrics
    pub fn get_prefetch_stats(&self) -> PrefetchStats {
        self.prefetch_predictor.get_metrics()
    }

    /// Shutdown policy engine and cleanup resources
    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        self.replacement_policies.shutdown()?;
        self.write_policy_manager.shutdown()?;
        self.prefetch_predictor.shutdown()?;
        Ok(())
    }
}

/// Policy performance statistics
#[derive(Debug, Clone)]
pub struct PolicyStats {
    pub hit_rates: [f64; 5],         // Per PolicyType
    pub latencies: [u64; 5],         // Average nanoseconds per PolicyType
    pub memory_efficiency: [f64; 5], // Efficiency per PolicyType
    pub switch_frequency: u32,       // Policy switches per hour
}

/// Write operation result
#[derive(Debug, Clone)]
pub struct WriteResult {
    pub success: bool,
    pub latency_ns: u64,
    pub tier: CacheTier,
}

/// Write operation statistics
#[derive(Debug, Clone)]
pub struct WriteStats {
    pub total_writes: u64,
    pub batched_writes: u64,
    pub average_latency_ns: u64,
    pub failure_count: u64,
}

/// Prefetch operation result
#[derive(Debug, Clone)]
pub struct PrefetchResult {
    pub successful_prefetches: usize,
    pub failed_prefetches: usize,
    pub total_latency_ns: u64,
}

/// Prefetch performance statistics
#[derive(Debug, Clone)]
pub struct PrefetchStats {
    pub hit_rate: f64,
    pub average_latency_ns: u64,
    pub successful_count: u64,
    pub failed_count: u64,
}

impl Default for PrefetchStats {
    fn default() -> Self {
        Self {
            hit_rate: 0.0,
            average_latency_ns: 0,
            successful_count: 0,
            failed_count: 0,
        }
    }
}

impl Default for PolicyStats {
    fn default() -> Self {
        Self {
            hit_rates: [0.0; 5],
            latencies: [0; 5],
            memory_efficiency: [0.0; 5],
            switch_frequency: 0,
        }
    }
}

impl Default for WriteResult {
    fn default() -> Self {
        Self {
            success: false,
            latency_ns: 0,
            tier: CacheTier::Hot,
        }
    }
}

impl Default for WriteStats {
    fn default() -> Self {
        Self {
            total_writes: 0,
            batched_writes: 0,
            average_latency_ns: 0,
            failure_count: 0,
        }
    }
}

impl Default for PrefetchResult {
    fn default() -> Self {
        Self {
            successful_prefetches: 0,
            failed_prefetches: 0,
            total_latency_ns: 0,
        }
    }
}
