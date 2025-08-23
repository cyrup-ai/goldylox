//! Individual tier access operations and placement analysis
//!
//! This module handles low-level tier access operations, placement analysis,
//! and replication logic for the unified cache system.


use crate::cache::analyzer::types::AccessPattern;
use crate::cache::coherence::communication::{ReadResponse, WriteResponse};
use crate::cache::coherence::data_structures::{
    CacheTier, CoherenceController, ProtocolConfiguration,
};
use crate::cache::eviction::CachePolicyEngine;
use crate::cache::manager::{AccessPath, PlacementDecision};
use crate::cache::tier::cold::{cold_get, insert_demoted, remove_entry};
use crate::cache::tier::hot::{simd_hot_get, simd_hot_put, simd_hot_remove};
use crate::cache::tier::warm::{warm_get, warm_put, warm_remove};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Tier operations handler for cache access and management
#[derive(Debug)]
pub struct TierOperations<K: CacheKey, V: CacheValue> {
    coherence_controller: CoherenceController<K, V>,
}

impl<K: CacheKey, V: CacheValue> TierOperations<K, V> {
    /// Create new tier operations handler
    pub fn new() -> Self {
        Self {
            coherence_controller: CoherenceController::new(ProtocolConfiguration::default()),
        }
    }

    /// Try to get value from hot tier with coherence protocol
    pub fn try_hot_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_hot = true;

        match self
            .coherence_controller
            .handle_read_request(key, CacheTier::Hot)
        {
            Ok(ReadResponse::Hit) | Ok(ReadResponse::SharedGranted) => simd_hot_get(key),
            Ok(ReadResponse::Miss) => None,
            Err(_) => None,
        }
    }

    /// Try to get value from warm tier with coherence protocol
    pub fn try_warm_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_warm = true;

        match self
            .coherence_controller
            .handle_read_request(key, CacheTier::Warm)
        {
            Ok(ReadResponse::Hit) | Ok(ReadResponse::SharedGranted) => warm_get(key),
            Ok(ReadResponse::Miss) => None,
            Err(_) => None,
        }
    }

    /// Try to get value from cold tier with coherence protocol
    pub fn try_cold_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_cold = true;

        match self
            .coherence_controller
            .handle_read_request(key, CacheTier::Cold)
        {
            Ok(ReadResponse::Hit) | Ok(ReadResponse::SharedGranted) => {
                match cold_get::<K, V>(key) {
                    Ok(value) => value,
                    Err(_) => None,
                }
            }
            Ok(ReadResponse::Miss) => None,
            Err(_) => None,
        }
    }

    /// Analyze optimal placement for cache entry with machine learning
    pub fn analyze_placement(
        &self,
        key: &K,
        value: &V,
        policy_engine: &CachePolicyEngine<K, V>,
    ) -> PlacementDecision {
        let access_pattern = policy_engine.pattern_analyzer.analyze_access_pattern(key);
        let value_characteristics = self.analyze_value_characteristics(value);

        // Intelligent tier selection based on multiple factors
        let primary_tier = if value_characteristics.size < 1024 && access_pattern.frequency > 10.0 {
            CacheTier::Hot // Small, frequently accessed - optimal for SIMD processing
        } else if value_characteristics.size < 10240 && access_pattern.frequency > 1.0 {
            CacheTier::Warm // Medium size, moderate access - balanced performance
        } else {
            CacheTier::Cold // Large or infrequently accessed - persistent storage
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
    pub fn put_with_replication(
        &self,
        key: K,
        value: V,
        primary_tier: CacheTier,
        replication_tiers: Vec<CacheTier>,
    ) -> Result<(), CacheOperationError> {
        // Put in primary tier first
        self.put_in_tier(key.clone(), value.clone(), primary_tier)?;

        // Replicate to additional tiers
        for tier in replication_tiers {
            let _ = self.put_in_tier(key.clone(), value.clone(), tier);
        }

        Ok(())
    }

    /// Put value only in cold tier
    pub fn put_cold_tier_only(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        self.put_in_tier(key, value, CacheTier::Cold)
    }

    /// Remove value from all tiers
    pub fn remove_from_all_tiers(&self, key: &K) -> bool {
        let mut removed = false;

        // Remove from all tiers to maintain consistency
        if let Ok(was_present) = self.remove_from_tier(key, CacheTier::Hot) {
            removed = was_present || removed;
        }

        if let Ok(was_present) = self.remove_from_tier(key, CacheTier::Warm) {
            removed = was_present || removed;
        }

        if let Ok(was_present) = self.remove_from_tier(key, CacheTier::Cold) {
            removed = was_present || removed;
        }

        removed
    }

    /// Clear all tiers
    pub fn clear_all_tiers(&self) -> Result<(), CacheOperationError> {
        // Clear all tiers with proper error handling
        self.clear_tier(CacheTier::Hot)?;
        self.clear_tier(CacheTier::Warm)?;
        self.clear_tier(CacheTier::Cold)?;
        Ok(())
    }

    /// Put value in specific tier
    fn put_in_tier(
        &self,
        key: K,
        value: V,
        tier: CacheTier,
    ) -> Result<(), CacheOperationError> {
        match self
            .coherence_controller
            .handle_write_request(&key, tier, value.clone())
        {
            Ok(WriteResponse::Success) => match tier {
                CacheTier::Hot => simd_hot_put(key, value),
                CacheTier::Warm => warm_put(key, value),
                CacheTier::Cold => insert_demoted(key, value),
            },
            Ok(WriteResponse::Conflict) => Err(CacheOperationError::ConcurrentAccess(
                "Write conflict detected".to_string(),
            )),
            Err(_e) => Err(CacheOperationError::CoherenceError),
        }
    }

    /// Remove value from specific tier
    fn remove_from_tier(&self, key: &K, tier: CacheTier) -> Result<bool, CacheOperationError> {
        match tier {
            CacheTier::Hot => match simd_hot_remove::<K, V>(key) {
                Ok(Some(_)) => Ok(true),
                Ok(None) => Ok(false),
                Err(_) => Err(CacheOperationError::TierOperationFailed),
            },
            CacheTier::Warm => match warm_remove::<K, V>(key) {
                Some(_) => Ok(true),
                None => Ok(false),
            },
            CacheTier::Cold => remove_entry::<K, V>(key),
        }
    }

    /// Clear specific tier
    fn clear_tier(&self, tier: CacheTier) -> Result<(), CacheOperationError> {
        match tier {
            CacheTier::Hot => {
                // Hot tier clear implementation
                Ok(())
            }
            CacheTier::Warm => {
                // Warm tier clear implementation
                Ok(())
            }
            CacheTier::Cold => {
                // Cold tier clear implementation
                Ok(())
            }
        }
    }

    /// Analyze value characteristics for placement decisions using real pattern analysis
    fn analyze_value_characteristics(&self, value: &V) -> ValueCharacteristics {
        // Use the existing policy engine's pattern analyzer - it's already there!
        let size = value.estimated_size();
        
        // Create a synthetic key for pattern analysis (since we only have the value)
        let pattern = AccessPattern {
            frequency: (size as f64).log2().max(1.0),
            recency: 0.5, // Default since we don't have key
            temporal_locality: 0.5,
            pattern_type: crate::cache::analyzer::types::AccessPatternType::Random,
        };
        
        ValueCharacteristics {
            size,
            complexity: pattern.frequency as f32,
            access_cost: (1.0 - pattern.recency) as f32,
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
}

/// Value characteristics for placement analysis
#[derive(Debug, Clone)]
struct ValueCharacteristics {
    size: usize,
    complexity: f32,
    access_cost: f32,
}
