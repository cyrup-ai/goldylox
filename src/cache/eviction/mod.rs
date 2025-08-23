//! Advanced cache eviction policies with machine learning-based decision making
//!
//! This module implements sophisticated cache replacement algorithms including
//! adaptive LRU/LFU, machine learning-based predictions, and intelligent prefetching.

pub mod ml_policies;
pub mod policy_engine;
pub mod prefetch;
pub mod traditional_policies;
pub mod types;
pub mod write_policies;

// Re-export core types
// Re-export ML policies
pub use ml_policies::{
    FeatureExtractor, MLModelMetrics, MLPerformanceMetrics, MLPredictivePolicy, SimpleNeuralNetwork,
};
// Re-export main policy engine
pub use policy_engine::{
    CachePolicyEngine, PolicyStats, PrefetchResult, PrefetchStats, WriteResult, WriteStats,
};
// Re-export prefetch functionality
pub use prefetch::{
    PrefetchPredictor,
};
// Re-export traditional policies
pub use traditional_policies::{
    ARCPolicy, AdaptiveLFUPolicy, AdaptiveLRUPolicy, PolicyMetrics, ReplacementPolicies,
    TwoQueuePolicy,
};
pub use types::{
    AccessEvent, AccessType, ConsistencyLevel, PolicyType, PrefetchRequest, WriteStrategy,
};
// Re-export write policies
pub use write_policies::{WriteBatchConfig, WritePolicyManager, WriteStatistics};

use super::coherence::CacheTier;
use super::config::CacheConfig;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Create new cache policy engine with configuration
pub fn create_policy_engine<K: CacheKey, V: CacheValue>(
    config: &CacheConfig,
) -> Result<CachePolicyEngine<K, V>, CacheOperationError> {
    CachePolicyEngine::new(config, PolicyType::default())
}

/// Create ML predictive policy with default configuration
pub fn create_ml_policy<K: CacheKey>() -> Result<MLPredictivePolicy<K>, CacheOperationError> {
    let config = CacheConfig::default();
    MLPredictivePolicy::new(&config)
}

/// Create write policy manager with configuration
pub fn create_write_policy_manager<K: CacheKey>(
    config: &CacheConfig,
) -> Result<WritePolicyManager<K>, CacheOperationError> {
    WritePolicyManager::new(config)
}

/// Create prefetch predictor with configuration
pub fn create_prefetch_predictor<K: CacheKey>(
    config: &CacheConfig,
) -> Result<PrefetchPredictor<K>, CacheOperationError> {
    PrefetchPredictor::new(config)
}

/// Convenience function to create complete eviction policy system
pub fn create_eviction_system<K: CacheKey, V: CacheValue>(
    config: &CacheConfig,
) -> Result<EvictionSystem<K, V>, CacheOperationError> {
    Ok(EvictionSystem {
        policy_engine: CachePolicyEngine::new(config, PolicyType::default())?,
        ml_policy: MLPredictivePolicy::new(config)?,
        write_manager: WritePolicyManager::new(config)?,
        prefetch_predictor: PrefetchPredictor::new(config)?,
    })
}

/// Complete eviction policy system
#[derive(Debug)]
pub struct EvictionSystem<K: CacheKey, V: CacheValue> {
    pub policy_engine: CachePolicyEngine<K, V>,
    pub ml_policy: MLPredictivePolicy<K>,
    pub write_manager: WritePolicyManager<K>,
    pub prefetch_predictor: PrefetchPredictor<K>,
}

impl<K: CacheKey, V: CacheValue> EvictionSystem<K, V> {
    /// Select replacement candidate using active policy
    pub fn select_replacement_candidate(&self, tier: CacheTier, candidates: &[K]) -> Option<K> {
        self.policy_engine
            .select_replacement_candidate(tier, candidates)
    }

    /// Record access event across all subsystems
    pub fn record_access(&self, event: AccessEvent<K>) {
        self.policy_engine.record_access(event.clone());
        self.ml_policy.record_access(&event);
        self.prefetch_predictor.record_access(&event);
    }

    /// Generate prefetch predictions
    pub fn generate_prefetch_predictions(
        &self,
        current_key: &K,
        access_history: &[AccessEvent<K>],
    ) -> Vec<PrefetchRequest<K>> {
        self.policy_engine
            .generate_prefetch_predictions(current_key, access_history)
    }

    /// Process write operation
    pub fn process_write_operation(
        &self,
        key: &K,
        tier: CacheTier,
    ) -> Result<WriteResult, CacheOperationError> {
        self.write_manager.process_write(key, tier)
    }

    /// Get comprehensive system statistics
    pub fn get_system_stats(&self) -> EvictionSystemStats {
        EvictionSystemStats {
            policy_stats: self.policy_engine.get_policy_stats(),
            ml_metrics: self.ml_policy.get_metrics(),
            write_stats: self.write_manager.get_statistics(),
            prefetch_stats: self.prefetch_predictor.get_metrics(),
        }
    }

    /// Shutdown entire eviction system
    pub fn shutdown(&self) -> Result<(), CacheOperationError> {
        self.policy_engine.shutdown()?;
        self.write_manager.shutdown()?;
        self.prefetch_predictor.shutdown()?;
        Ok(())
    }
}

/// Comprehensive eviction system statistics
#[derive(Debug, Clone)]
pub struct EvictionSystemStats {
    pub policy_stats: PolicyStats,
    pub ml_metrics: MLPerformanceMetrics,
    pub write_stats: WriteStats,
    pub prefetch_stats: PrefetchStats,
}

impl Default for EvictionSystemStats {
    fn default() -> Self {
        Self {
            policy_stats: PolicyStats::default(),
            ml_metrics: MLPerformanceMetrics {
                accuracy: 0.5,
                training_iterations: 0,
                model_loss: 1.0,
            },
            write_stats: WriteStats::default(),
            prefetch_stats: PrefetchStats::default(),
        }
    }
}
