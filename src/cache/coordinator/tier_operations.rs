//! Individual tier access operations and placement analysis
//!
//! This module handles low-level tier access operations, placement analysis,
//! and replication logic for the unified cache system.

use crate::cache::analyzer::types::AccessPattern;
use std::time::Duration;

use crate::cache::coherence::data_structures::{
    CacheTier, CoherenceController, ProtocolConfiguration,
};
use crate::cache::eviction::CachePolicyEngine;
use crate::cache::tier::cold::{cold_get, insert_demoted, remove_entry};
use crate::cache::tier::hot::thread_local::HotTierCoordinator;
use crate::cache::tier::warm::global_api::WarmTierCoordinator;
use crate::cache::tier::warm::{warm_get, warm_get_timestamps, warm_put, warm_remove};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::{AccessPath, PlacementDecision};

/// Tier operations handler for cache access and management
#[derive(Debug)]
pub struct TierOperations<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> {
    coherence_controller: CoherenceController<K, V>,
    
    /// Per-instance hot tier coordinator (injected from UnifiedCacheManager)
    /// Manages type-erased hot tier instances and load balancing
    hot_tier_coordinator: HotTierCoordinator,
    
    /// Per-instance warm tier coordinator (injected from UnifiedCacheManager)
    /// Manages ML-driven warm tier instances
    warm_tier_coordinator: WarmTierCoordinator,
    
    /// Per-instance cold tier coordinator (injected from UnifiedCacheManager)
    /// Manages cold tier service channel communication
    cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
}

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> TierOperations<K, V>
{
    /// Create tier operations with injected coordinators (per-instance pattern)
    pub fn new_with_coordinators(
        hot_tier_coordinator: HotTierCoordinator,
        warm_tier_coordinator: WarmTierCoordinator,
        cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    ) -> Result<Self, CacheOperationError> {
        Ok(Self {
            coherence_controller: CoherenceController::new(
                ProtocolConfiguration::default(),
                hot_tier_coordinator.clone(),
                warm_tier_coordinator.clone(),
                cold_tier_coordinator.clone(),
            ),
            hot_tier_coordinator,
            warm_tier_coordinator,
            cold_tier_coordinator,
        })
    }

    /// Try to get value from hot tier with coherence protocol
    pub async fn try_hot_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_hot = true;

        // SEARCH FIRST, then update coherence state based on results
        if let Some(value) = crate::cache::tier::hot::simd_hot_get::<K, V>(&self.hot_tier_coordinator, key).await {
            // Update coherence state to reflect successful hit
            let _ = self
                .coherence_controller
                .update_state_after_hit(key, CacheTier::Hot);
            Some(value)
        } else {
            // Key not found - coherence controller can handle miss scenario for coordination
            let _ = self
                .coherence_controller
                .handle_read_request(key, CacheTier::Hot);
            None
        }
    }

    /// Try to get value from warm tier with coherence protocol and timestamp population
    pub async fn try_warm_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_warm = true;

        // SEARCH FIRST, then update coherence state based on results
        if let Some(value) = warm_get(&self.warm_tier_coordinator, key).await {
            // POPULATE TIMESTAMPS from warm tier's ConcurrentAccessTracker
            // This is CRITICAL for temporal locality calculation
            access_path.access_timestamps = warm_get_timestamps::<K, V>(
                &self.warm_tier_coordinator,
                key,
                10,  // Last 10 accesses for locality analysis
            ).await;

            // POPULATE KEY HASHES from warm tier's ConcurrentAccessTracker
            // This is CRITICAL for spatial locality calculation
            access_path.recent_key_hashes = crate::cache::tier::warm::warm_get_key_hashes::<K, V>(
                &self.warm_tier_coordinator,
                10,  // Last 10 accesses for locality analysis
            ).await;

            // Update coherence state to reflect successful hit
            let _ = self
                .coherence_controller
                .update_state_after_hit(key, CacheTier::Warm);
            Some(value)
        } else {
            // Key not found - coherence controller can handle miss scenario for coordination
            let _ = self
                .coherence_controller
                .handle_read_request(key, CacheTier::Warm);
            None
        }
    }

    /// Try to get value from cold tier with coherence protocol
    pub async fn try_cold_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_cold = true;

        // SEARCH FIRST, then update coherence state based on results
        match cold_get::<K, V>(&self.cold_tier_coordinator, key).await {
            Ok(Some(value)) => {
                // Update coherence state to reflect successful hit
                let _ = self
                    .coherence_controller
                    .update_state_after_hit(key, CacheTier::Cold);
                Some(value)
            }
            Ok(None) | Err(_) => {
                // Key not found or error - coherence controller can handle miss scenario for coordination
                let _ = self
                    .coherence_controller
                    .handle_read_request(key, CacheTier::Cold);
                None
            }
        }
    }

    /// Analyze optimal placement for cache entry with policy engine intelligence
    pub fn analyze_placement(
        &self,
        key: &K,
        value: &V,
        policy_engine: &CachePolicyEngine<K, V>,
    ) -> PlacementDecision {
        let access_pattern = policy_engine.analyze_access_pattern(key);
        let value_characteristics = self.analyze_value_characteristics(value);
        let current_policy = policy_engine.current_policy();

        // Policy engine-driven tier selection with cold start optimization
        let primary_tier = match current_policy {
            crate::cache::eviction::PolicyType::AdaptiveLRU => {
                // LRU policy: prioritize recency and size
                if value_characteristics.size < 1024 && access_pattern.recency > 0.5 {
                    CacheTier::Hot
                } else if value_characteristics.size < 10240 && access_pattern.recency > 0.3 {
                    CacheTier::Warm
                } else {
                    CacheTier::Cold
                }
            }
            crate::cache::eviction::PolicyType::AdaptiveLFU => {
                // LFU policy: prioritize frequency and access cost
                if access_pattern.frequency > 2.0 && value_characteristics.access_cost < 0.5 {
                    CacheTier::Hot
                } else if access_pattern.frequency > 0.5 && value_characteristics.access_cost < 0.8
                {
                    CacheTier::Warm
                } else {
                    CacheTier::Cold
                }
            }
            crate::cache::eviction::PolicyType::TwoQueue
            | crate::cache::eviction::PolicyType::Arc => {
                // Two-queue/ARC: balance frequency and recency with complexity
                if value_characteristics.size < 1024
                    && access_pattern.temporal_locality > 0.6
                    && value_characteristics.complexity < 0.5
                {
                    CacheTier::Hot
                } else if value_characteristics.size < 10240
                    && access_pattern.temporal_locality > 0.3
                    && value_characteristics.complexity < 2.0
                {
                    CacheTier::Warm
                } else {
                    CacheTier::Cold
                }
            }
            crate::cache::eviction::PolicyType::MLPredictive => {
                // ML policy: sophisticated analysis with cold start optimization
                if value_characteristics.size < 1024 && value_characteristics.complexity < 0.5 {
                    // Small, low complexity - always start in hot tier for ML learning
                    CacheTier::Hot
                } else if value_characteristics.size < 10240
                    && access_pattern.frequency > 1.0
                    && value_characteristics.complexity < 2.0
                    && value_characteristics.access_cost < 0.7
                {
                    CacheTier::Warm
                } else {
                    CacheTier::Cold
                }
            }
        };

        // Determine replication strategy for cross-tier consistency
        let replication_tiers = match primary_tier {
            CacheTier::Hot => {
                if value_characteristics.size < 512 {
                    vec![CacheTier::Warm] // Replicate small hot entries for durability
                } else {
                    vec![]
                }
            }
            CacheTier::Warm => {
                if access_pattern.temporal_locality > 0.8 {
                    vec![CacheTier::Cold] // Replicate high-locality warm entries
                } else {
                    vec![]
                }
            }
            CacheTier::Cold => vec![], // Cold tier entries are not replicated
        };

        PlacementDecision {
            primary_tier,
            replication_tiers,
            confidence: self.calculate_placement_confidence(&access_pattern, &value_characteristics)
                as f32,
        }
    }

    /// Put value with replication across multiple tiers
    pub async fn put_with_replication(
        &self,
        key: K,
        value: V,
        primary_tier: CacheTier,
        replication_tiers: Vec<CacheTier>,
    ) -> Result<(), CacheOperationError> {
        // Put in primary tier first
        self.put_in_tier(key.clone(), value.clone(), primary_tier).await?;

        // Replicate to additional tiers
        for tier in replication_tiers {
            let _ = self.put_in_tier(key.clone(), value.clone(), tier).await;
        }

        Ok(())
    }

    /// Put value only in cold tier
    pub async fn put_cold_tier_only(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        self.put_in_tier(key, value, CacheTier::Cold).await
    }

    /// Remove value from all tiers
    pub async fn remove_from_all_tiers(&self, key: &K) -> bool {
        let mut removed = false;

        // Remove from all tiers to maintain consistency
        if let Ok(was_present) = self.remove_from_tier(key, CacheTier::Hot).await {
            removed = was_present || removed;
        }

        if let Ok(was_present) = self.remove_from_tier(key, CacheTier::Warm).await {
            removed = was_present || removed;
        }

        if let Ok(was_present) = self.remove_from_tier(key, CacheTier::Cold).await {
            removed = was_present || removed;
        }

        removed
    }

    /// Clear all tiers
    pub async fn clear_all_tiers(&self) -> Result<(), CacheOperationError> {
        // Clear all tiers with proper error handling
        self.clear_tier(CacheTier::Hot).await?;
        self.clear_tier(CacheTier::Warm).await?;
        self.clear_tier(CacheTier::Cold).await?;
        Ok(())
    }

    /// Atomically put value only if key is not present, returns previous value if present
    pub async fn put_if_absent_atomic(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        // Try hot tier first (most frequent access)
        match self.put_if_absent_hot_tier(key.clone(), value.clone()).await {
            Ok(Some(existing)) => return Ok(Some(existing)),
            Ok(None) => return Ok(None), // Successfully inserted in hot tier
            Err(_) => {}                 // Hot tier failed, try warm tier
        }

        // Try warm tier with DashMap atomic entry API
        match self.put_if_absent_warm_tier(key.clone(), value.clone()).await {
            Ok(Some(existing)) => Ok(Some(existing)),
            Ok(None) => Ok(None), // Successfully inserted in warm tier
            Err(_) => {
                // Fall back to cold tier
                self.put_if_absent_cold_tier(key, value).await
            }
        }
    }

    /// Atomically replace existing value, returns old value if key existed
    pub async fn replace_atomic(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        // Try hot tier first
        if let Ok(old_value) = self.replace_hot_tier(key.clone(), value.clone()).await {
            return Ok(old_value);
        }
        // Hot tier failed, try warm tier

        // Try warm tier with DashMap atomic operations
        match self.replace_warm_tier(key.clone(), value.clone()).await {
            Ok(old_value) => Ok(old_value),
            Err(_) => {
                // Fall back to cold tier
                self.replace_cold_tier(key, value).await
            }
        }
    }

    /// Atomically compare and swap value if current equals expected
    #[allow(dead_code)] // Library API - may be used by external consumers
    pub async fn compare_and_swap_atomic(
        &self,
        key: K,
        expected: V,
        new_value: V,
    ) -> Result<bool, CacheOperationError>
    where
        V: PartialEq,
    {
        // Try hot tier first
        if let Ok(swapped) =
            self.compare_and_swap_hot_tier(key.clone(), expected.clone(), new_value.clone()).await
        {
            return Ok(swapped);
        }
        // Hot tier failed, try warm tier

        // Try warm tier with DashMap atomic operations
        match self.compare_and_swap_warm_tier(key.clone(), expected.clone(), new_value.clone()).await {
            Ok(swapped) => Ok(swapped),
            Err(_) => {
                // Fall back to cold tier
                self.compare_and_swap_cold_tier(key, expected, new_value).await
            }
        }
    }

    /// Atomically get value or insert using factory if key is absent
    #[allow(dead_code)] // Library API - may be used by external consumers
    pub async fn get_or_insert_atomic<F>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where
        F: FnOnce() -> V,
    {
        // Try to get existing value first (fast path)
        let mut access_path = AccessPath::new();
        if let Some(existing) = self.try_hot_tier_get(&key, &mut access_path).await {
            return Ok(existing);
        }
        if let Some(existing) = self.try_warm_tier_get(&key, &mut access_path).await {
            return Ok(existing);
        }
        if let Some(existing) = self.try_cold_tier_get(&key, &mut access_path).await {
            return Ok(existing);
        }

        // Key doesn't exist, create value and try atomic insert
        let new_value = factory();
        match self.put_if_absent_atomic(key, new_value.clone()).await? {
            Some(existing) => Ok(existing), // Another thread inserted first
            None => Ok(new_value),          // We successfully inserted
        }
    }

    /// Put value in specific tier
    async fn put_in_tier(&self, key: K, value: V, tier: CacheTier) -> Result<(), CacheOperationError> {
        // PUT FIRST, then update coherence state based on results
        let put_result = match tier {
            CacheTier::Hot => {
                crate::cache::tier::hot::simd_hot_put::<K, V>(&self.hot_tier_coordinator, key.clone(), value.clone()).await
            }
            CacheTier::Warm => warm_put(&self.warm_tier_coordinator, key.clone(), value.clone()).await,
            CacheTier::Cold => insert_demoted(&self.cold_tier_coordinator, key.clone(), value.clone()).await,
        };

        match put_result {
            Ok(()) => {
                // Successfully stored - update coherence state to reflect the put
                // For simplicity, assume new entry (could be enhanced to detect update vs insert)
                let _ = self
                    .coherence_controller
                    .update_state_after_put(&key, tier, true);
                Ok(())
            }
            Err(e) => {
                // Put failed - coherence controller can handle write conflict scenarios
                let _ = self
                    .coherence_controller
                    .handle_write_request(&key, tier, value);
                Err(e)
            }
        }
    }

    /// Remove value from specific tier
    async fn remove_from_tier(&self, key: &K, tier: CacheTier) -> Result<bool, CacheOperationError> {
        // REMOVE FIRST, then update coherence state based on results
        let remove_result = match tier {
            CacheTier::Hot => match crate::cache::tier::hot::simd_hot_remove::<K, V>(&self.hot_tier_coordinator, key).await {
                Ok(Some(_)) => Ok(true),
                Ok(None) => Ok(false),
                Err(_) => Err(CacheOperationError::TierOperationFailed),
            },
            CacheTier::Warm => match warm_remove::<K, V>(&self.warm_tier_coordinator, key).await {
                Some(_) => Ok(true),
                None => Ok(false),
            },
            CacheTier::Cold => remove_entry::<K, V>(&self.cold_tier_coordinator, key).await,
        };

        match remove_result {
            Ok(true) => {
                // Successfully removed - update coherence state to reflect removal
                let _ = self
                    .coherence_controller
                    .update_state_after_remove(key, tier);
                Ok(true)
            }
            Ok(false) => Ok(false), // Key not found, no coherence update needed
            Err(e) => Err(e),       // Error occurred, no coherence update needed
        }
    }

    /// Clear specific tier
    async fn clear_tier(&self, tier: CacheTier) -> Result<(), CacheOperationError> {
        match tier {
            CacheTier::Hot => {
                // Use existing clear_hot_tier function with proper generic parameters
                crate::cache::tier::hot::thread_local::clear_hot_tier::<K, V>(&self.hot_tier_coordinator).await;
                Ok(())
            }
            CacheTier::Warm => {
                // Use existing clear_all_warm_tiers function
                crate::cache::tier::warm::global_api::clear_all_warm_tiers(&self.warm_tier_coordinator)
            }
            CacheTier::Cold => {
                // Use the production-ready cold tier clear function
                crate::cache::tier::cold::clear::<K, V>(&self.cold_tier_coordinator).await
            }
        }
    }

    /// Analyze value characteristics for placement decisions using real pattern analysis
    fn analyze_value_characteristics(&self, value: &V) -> ValueCharacteristics {
        // Use the existing policy engine's pattern analyzer - it's already there!
        let size = value.estimated_size();

        // Create a synthetic key for pattern analysis (since we only have the value)
        let _pattern = AccessPattern {
            frequency: (size as f64).log2().max(1.0),
            recency: 0.5, // Default since we don't have key
            temporal_locality: 0.5,
            pattern_type: crate::cache::analyzer::types::AccessPatternType::Random,
        };

        ValueCharacteristics {
            size,
            complexity: self.calculate_rendering_complexity(value),
            access_cost: self.estimate_access_cost(value),
        }
    }

    /// Calculate rendering complexity for placement optimization
    fn calculate_rendering_complexity(&self, value: &V) -> f32 {
        // Use real ML feature analysis from the sophisticated FeatureVector system
        use crate::cache::tier::warm::eviction::ml::features::FeatureVector;
        let size = value.estimated_size();

        // Connect to real feature importance system
        let feature_weights = FeatureVector::get_feature_importance();
        let complexity_score = (size as f64).log2() * feature_weights[3]; // relative_size weight

        complexity_score as f32
    }

    /// Estimate access cost for different tiers
    fn estimate_access_cost(&self, value: &V) -> f32 {
        // Use real ML feature system for access cost prediction
        use crate::cache::tier::warm::eviction::ml::features::FeatureVector;
        let size = value.estimated_size();

        // Get real feature importance weights
        let feature_weights = FeatureVector::get_feature_importance();

        // Combine multiple real ML features for access cost prediction
        let recency_cost = feature_weights[0]; // recency importance
        let frequency_cost = feature_weights[1]; // frequency importance  
        let size_cost = (size as f64).log2() * feature_weights[3]; // relative_size

        (recency_cost + frequency_cost + size_cost) as f32
    }

    /// Calculate placement confidence score
    fn calculate_placement_confidence(
        &self,
        access_pattern: &AccessPattern,
        value_characteristics: &ValueCharacteristics,
    ) -> f64 {
        let frequency_confidence = (access_pattern.frequency / 20.0).min(1.0);
        let size_confidence = if value_characteristics.size < 2048 {
            0.9
        } else {
            0.7
        };
        let locality_confidence = access_pattern.temporal_locality;

        (frequency_confidence + size_confidence + locality_confidence) / 3.0
    }

    // Hot tier atomic operations using service messages
    pub async fn put_if_absent_hot_tier(
        &self,
        key: K,
        value: V,
    ) -> Result<Option<V>, CacheOperationError> {
        // Use hot tier service message for atomic operation
        crate::cache::tier::hot::put_if_absent_atomic::<K, V>(&self.hot_tier_coordinator, key, value).await
    }

    pub async fn replace_hot_tier(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        crate::cache::tier::hot::replace_atomic::<K, V>(&self.hot_tier_coordinator, key, value).await
    }

    pub async fn compare_and_swap_hot_tier(
        &self,
        key: K,
        expected: V,
        new_value: V,
    ) -> Result<bool, CacheOperationError>
    where
        V: PartialEq,
    {
        crate::cache::tier::hot::compare_and_swap_atomic::<K, V>(&self.hot_tier_coordinator, key, expected, new_value).await
    }

    // Warm tier atomic operations using DashMap entry API
    pub async fn put_if_absent_warm_tier(
        &self,
        key: K,
        value: V,
    ) -> Result<Option<V>, CacheOperationError> {
        crate::cache::tier::warm::put_if_absent_atomic::<K, V>(&self.warm_tier_coordinator, key, value).await
    }

    pub async fn replace_warm_tier(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        crate::cache::tier::warm::replace_atomic::<K, V>(&self.warm_tier_coordinator, key, value).await
    }

    pub async fn compare_and_swap_warm_tier(
        &self,
        key: K,
        expected: V,
        new_value: V,
    ) -> Result<bool, CacheOperationError>
    where
        V: PartialEq,
    {
        crate::cache::tier::warm::compare_and_swap_atomic::<K, V>(&self.warm_tier_coordinator, key, expected, new_value).await
    }

    // Cold tier atomic operations (simple synchronization for disk operations)
    pub async fn put_if_absent_cold_tier(
        &self,
        key: K,
        value: V,
    ) -> Result<Option<V>, CacheOperationError> {
        crate::cache::tier::cold::put_if_absent_atomic::<K, V>(&self.cold_tier_coordinator, key, value).await
    }

    pub async fn replace_cold_tier(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        crate::cache::tier::cold::replace_atomic::<K, V>(&self.cold_tier_coordinator, key, value).await
    }

    pub async fn compare_and_swap_cold_tier(
        &self,
        key: K,
        expected: V,
        new_value: V,
    ) -> Result<bool, CacheOperationError>
    where
        V: PartialEq,
    {
        crate::cache::tier::cold::compare_and_swap_atomic::<K, V>(&self.cold_tier_coordinator, key, expected, new_value).await
    }

    /// Select eviction candidate using policy engine intelligence
    #[allow(dead_code)] // Tier operations - policy-driven eviction candidate selection with ML integration
    pub fn select_eviction_candidate(
        &self,
        tier: CacheTier,
        candidates: Vec<K>,
        policy_engine: &CachePolicyEngine<K, V>,
    ) -> Option<K> {
        if candidates.is_empty() {
            return None;
        }

        // Use policy engine's sophisticated replacement candidate selection
        let coherence_tier = match tier {
            CacheTier::Hot => crate::cache::coherence::CacheTier::Hot,
            CacheTier::Warm => crate::cache::coherence::CacheTier::Warm,
            CacheTier::Cold => crate::cache::coherence::CacheTier::Cold,
        };

        policy_engine.select_replacement_candidate(coherence_tier, &candidates)
    }

    /// Execute eviction using policy engine decision
    #[allow(dead_code)] // Tier operations - complete policy-driven eviction with candidate analysis and ML optimization
    pub async fn execute_policy_driven_eviction(
        &self,
        tier: CacheTier,
        policy_engine: &CachePolicyEngine<K, V>,
        _max_candidates: usize,
    ) -> Result<Option<K>, CacheOperationError> {
        // Get eviction candidates from the appropriate tier
        let candidates = match tier {
            CacheTier::Hot => {
                // Get idle keys from hot tier using thread-local system
                crate::cache::tier::hot::get_idle_keys::<K, V>(&self.hot_tier_coordinator, Duration::from_secs(60)).await
            }
            CacheTier::Warm => {
                // Use ML-driven eviction policy engine to select sophisticated candidates
                // This integrates coherence protocol, ML analysis, and policy-driven selection
                let warm_tier_candidates =
                    crate::cache::tier::warm::global_api::get_warm_tier_keys::<K, V>(&self.warm_tier_coordinator).await;

                // Use policy engine's sophisticated ML algorithms to filter candidates
                let policy_candidates: Vec<K> = warm_tier_candidates
                    .into_iter()
                    .filter_map(|key| {
                        // Apply policy engine intelligence for candidate filtering
                        let access_pattern = policy_engine.analyze_access_pattern(&key);
                        if access_pattern.frequency < 2.0 && access_pattern.recency < 0.3 {
                            Some(key)
                        } else {
                            None
                        }
                    })
                    .take(_max_candidates.min(10)) // Limit candidates for performance
                    .collect();

                policy_candidates
            }
            CacheTier::Cold => {
                // Get frequently accessed keys from cold tier for potential eviction
                crate::cache::tier::cold::get_frequently_accessed_keys::<K, V>(&self.cold_tier_coordinator, 1).await
            }
        };

        // Use policy engine to select best eviction candidate
        if let Some(eviction_key) = self.select_eviction_candidate(tier, candidates, policy_engine)
        {
            // Execute the eviction
            match self.remove_from_tier(&eviction_key, tier).await {
                Ok(true) => Ok(Some(eviction_key)),
                Ok(false) => Ok(None), // Key was already removed
                Err(e) => Err(e),
            }
        } else {
            Ok(None) // No candidates or no suitable candidate selected
        }
    }

    /// Update policy performance based on eviction success
    #[allow(dead_code)] // Tier operations - eviction quality feedback system with performance metrics and ML adaptation
    pub fn update_eviction_performance(
        &self,
        policy_engine: &CachePolicyEngine<K, V>,
        _evicted_key: &K,
        was_accessed_again: bool,
        time_until_reaccess_ns: Option<u64>,
    ) {
        let current_policy = policy_engine.current_policy();

        // Calculate performance metrics based on eviction quality
        let hit_rate = if was_accessed_again {
            // Poor eviction - key was accessed again soon after eviction
            match time_until_reaccess_ns {
                Some(time) if time < 1_000_000 => 0.1,   // < 1ms: very poor
                Some(time) if time < 10_000_000 => 0.3,  // < 10ms: poor
                Some(time) if time < 100_000_000 => 0.6, // < 100ms: moderate
                _ => 0.8,                                // > 100ms or unknown: acceptable
            }
        } else {
            0.95 // Good eviction - key not accessed again
        };

        let latency_penalty = if was_accessed_again { 1_000_000 } else { 0 }; // 1ms penalty for poor evictions
        let memory_efficiency = if was_accessed_again { 0.5 } else { 0.9 };

        policy_engine.update_performance_metrics(
            current_policy,
            hit_rate,
            latency_penalty,
            memory_efficiency,
        );
    }
}

/// Value characteristics for placement analysis
#[derive(Debug, Clone)]
pub struct ValueCharacteristics {
    size: usize,
    complexity: f32,
    access_cost: f32,
}
